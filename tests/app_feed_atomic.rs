mod app_feed_support;
use app_feed_support::Installed;
use panorama::app_feed::{
    fs::{self, Home},
    journal::Journal,
    receipt::{FEED, FEED_LIMIT, Ownership, Receipt, SMALL},
};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn frame(n: u64) -> Vec<u8> {
    let mut feed: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/app-feed/degraded.json")).unwrap();
    feed["frame"] = json!(n);
    feed["payload"] = json!("x".repeat(32768));
    serde_json::to_vec(&feed).unwrap()
}

#[test]
fn atomic_readers_see_only_complete_old_or_new_frames() {
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let receipt = Receipt::read(&home).unwrap();
    let lifetime = receipt.lock_file(&home).unwrap();
    fs::lock(&lifetime, Duration::ZERO).unwrap();
    let frames = Arc::new((0..8).map(frame).collect::<Vec<_>>());
    let mut journal = Journal::open(&home, &receipt).unwrap();
    journal.publish(&home, &receipt, &frames[0]).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let allowed = frames.clone();
    let path = installed.app.join(FEED);
    let reader = std::thread::spawn(move || {
        let mut read = 0;
        while stop.load(Ordering::Relaxed) {
            let bytes = std::fs::read(&path).unwrap();
            assert!(allowed.contains(&bytes));
            let _: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            read += 1;
        }
        read
    });
    for bytes in frames.iter().skip(1) {
        journal.publish(&home, &receipt, bytes).unwrap();
    }
    running.store(false, Ordering::Relaxed);
    assert!(reader.join().unwrap() > 0);
    let owner = Ownership::read(&home, &receipt).unwrap().unwrap().0;
    assert_eq!(
        owner.feed_sha256.as_deref(),
        Some(fs::digest(frames.last().unwrap()).as_str())
    );
    assert!(owner.pending_feed_sha256.is_none());
}

#[test]
fn personal_feed_and_unproved_temporary_files_are_not_replaced_or_adopted() {
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let receipt = Receipt::read(&home).unwrap();
    let lifetime = receipt.lock_file(&home).unwrap();
    fs::lock(&lifetime, Duration::ZERO).unwrap();
    let mut journal = Journal::open(&home, &receipt).unwrap();
    journal.publish(&home, &receipt, &frame(0)).unwrap();
    std::fs::write(installed.app.join(FEED), b"personal content").unwrap();
    assert!(journal.publish(&home, &receipt, &frame(1)).is_err());
    assert_eq!(
        std::fs::read(installed.app.join(FEED)).unwrap(),
        b"personal content"
    );
    assert!(Journal::open(&home, &receipt).is_err());
    let temp = installed.app.join(receipt.path(".ownership.tmp"));
    std::fs::write(&temp, b"personal staging").unwrap();
    assert!(Journal::open(&home, &receipt).is_err());
    assert_eq!(std::fs::read(&temp).unwrap(), b"personal staging");
}

#[test]
fn pending_digest_recovers_both_sides_of_the_publication_crash_window() {
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let receipt = Receipt::read(&home).unwrap();
    let lifetime = receipt.lock_file(&home).unwrap();
    fs::lock(&lifetime, Duration::ZERO).unwrap();
    let mut journal = Journal::open(&home, &receipt).unwrap();
    journal.publish(&home, &receipt, &frame(0)).unwrap();
    let (mut owner, known) = Ownership::read(&home, &receipt).unwrap().unwrap();
    let next = frame(1);
    owner.pending_feed_sha256 = Some(fs::digest(&next));
    owner.write(&home, &receipt, Some(&known)).unwrap();
    // Crash before feed rename: complete staging is proven, old feed stays authoritative.
    home.replace(
        &receipt.feed_temp(),
        "data/panorama/staging-fixture",
        &next,
        None,
    )
    .unwrap();
    let recovered = Journal::open(&home, &receipt).unwrap();
    assert_eq!(
        recovered.owner.feed_sha256.as_deref(),
        Some(fs::digest(&frame(0)).as_str())
    );
    assert!(!installed.app.join(receipt.feed_temp()).exists());
    // Crash after rename: pending digest proves the complete new feed before commit.
    let (mut owner, known) = Ownership::read(&home, &receipt).unwrap().unwrap();
    owner.pending_feed_sha256 = Some(fs::digest(&next));
    owner.write(&home, &receipt, Some(&known)).unwrap();
    let old = home.read(FEED, FEED_LIMIT, 0o600).unwrap().unwrap();
    home.replace(FEED, &receipt.feed_temp(), &next, Some(&old))
        .unwrap();
    let recovered = Journal::open(&home, &receipt).unwrap();
    assert_eq!(
        recovered.owner.feed_sha256.as_deref(),
        Some(fs::digest(&next).as_str())
    );
    assert!(recovered.owner.pending_feed_sha256.is_none());
    assert!(
        home.read(&receipt.path("ownership.json"), SMALL, 0o600)
            .unwrap()
            .is_some()
    );
}

#[test]
fn disabled_control_between_staging_and_rename_preserves_the_last_feed() {
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    let mut journal = Journal::open(&home, &r).unwrap();
    journal.publish(&home, &r, &frame(1)).unwrap();
    let old = home.read(FEED, FEED_LIMIT, 0o600).unwrap().unwrap();
    let result = home.replace_checked(FEED, &r.feed_temp(), &frame(2), Some(&old), || {
        r.disable(&home)?;
        r.active(&home)
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(installed.app.join(FEED)).unwrap(), old.bytes);
    assert!(!installed.app.join(r.feed_temp()).exists());
}
