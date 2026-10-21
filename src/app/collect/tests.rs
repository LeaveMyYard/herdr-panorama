//! The collect thread: one collect at a time, a cleaned swarm, a panic as a failed refresh.

use super::*;
use std::time::Duration;

fn recv(rx: &mut mpsc::UnboundedReceiver<Collected>) -> Collected {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap()
    })
}

#[test]
fn one_collect_at_a_time_and_its_swarm_is_cleaned() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut w = Worker::spawn(
        || {
            Box::new(|| Swarm {
                errors: vec!["bad \u{1b}[31mrecord\u{7}".into()],
                docked: Some("w1:p3".into()),
                ..Swarm::default()
            }) as Source
        },
        tx,
    )
    .unwrap();
    assert!(w.request());
    assert!(!w.request()); // one is running: the tick is skipped
    match recv(&mut rx) {
        Collected::Swarm(sw) => {
            assert!(!sw.errors[0].contains('\u{1b}') && !sw.errors[0].contains('\u{7}'));
            assert_eq!(sw.docked.as_deref(), Some("w1:p3")); // kept through the cleaning
        }
        other => panic!("{other:?}"),
    }
    w.done();
    assert!(w.request());
    assert!(matches!(recv(&mut rx), Collected::Swarm(_)));
}

#[test]
fn a_panicking_collect_is_a_failed_refresh_and_the_next_one_still_runs() {
    let _hook = crate::ui::guard::hook_lock();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // keep the test's output quiet
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut n = 0;
    let mut w = Worker::spawn(
        move || {
            Box::new(move || {
                n += 1;
                assert!(n != 1, "bad record");
                Swarm::default()
            }) as Source
        },
        tx,
    )
    .unwrap();
    w.request();
    let first = recv(&mut rx);
    w.done();
    w.request();
    let second = recv(&mut rx);
    std::panic::set_hook(prev);
    assert!(
        matches!(&first, Collected::Failed(m) if m.contains("bad record")),
        "{first:?}"
    );
    assert!(matches!(second, Collected::Swarm(_)));
}

#[test]
fn a_source_that_cannot_start_fails_every_collect() {
    let _hook = crate::ui::guard::hook_lock();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut w = Worker::spawn(|| -> Source { panic!("no herdr") }, tx).unwrap();
    let started = recv(&mut rx);
    w.request();
    let asked = recv(&mut rx);
    std::panic::set_hook(prev);
    assert!(
        matches!(&started, Collected::Failed(m) if m.contains("no herdr")),
        "{started:?}"
    );
    assert!(matches!(asked, Collected::Failed(_)));
}
