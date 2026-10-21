//! The app's panic hook and the build that lets it contain panics.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What reached the default hook (other tests' panics may too: counted by message).
static NEXT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static RESTORED: AtomicUsize = AtomicUsize::new(0);

fn spoke(what: &str) -> bool {
    NEXT.lock().unwrap().iter().any(|m| m.contains(what))
}

fn restore() {
    RESTORED.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn the_log_keeps_its_last_16_kb() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_path(dir.path());
    log(&path, "first");
    log(&path, "second");
    assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    log(&path, &"x".repeat(crash::LOG_BYTES));
    let kept = fs::read_to_string(&path).unwrap();
    assert_eq!(kept.len(), crash::LOG_BYTES);
    assert!(!kept.contains("first"));
}

#[test]
fn a_contained_panic_only_goes_to_the_log_any_other_restores_and_speaks() {
    let _hook = crate::ui::guard::hook_lock();
    let dir = tempfile::tempdir().unwrap();
    let path = log_path(dir.path());
    let prev = std::panic::take_hook();
    std::panic::set_hook(hook(
        path.clone(),
        restore,
        Box::new(|info| {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned());
            NEXT.lock().unwrap().push(msg.unwrap_or_default());
        }),
    ));
    // on the collect thread: contained
    let t = std::thread::Builder::new()
        .name(THREAD.into())
        .spawn(|| std::panic::catch_unwind(|| panic!("in a collect")).is_err())
        .unwrap();
    assert!(t.join().unwrap());
    let (contained_spoke, r1) = (spoke("in a collect"), RESTORED.load(Ordering::SeqCst));
    // anywhere else: the terminal comes back, then the default hook
    let t = std::thread::spawn(|| std::panic::catch_unwind(|| panic!("in the view")).is_err());
    assert!(t.join().unwrap());
    let (view_spoke, r2) = (spoke("in the view"), RESTORED.load(Ordering::SeqCst));
    std::panic::set_hook(prev);
    assert!(!contained_spoke, "a contained panic reached the terminal");
    assert!(
        view_spoke && r2 > r1,
        "an uncontained panic restores and speaks"
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("in a collect") && text.contains("in the view"));
}

#[test]
fn no_profile_aborts_on_panic() {
    // containment (the collector's per-orchestrator catch_unwind, the collect thread's) needs
    // unwinding: a `panic = "abort"` anywhere in the workspace's manifest would turn it off
    let manifest = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let compact: String = manifest.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(!compact.contains("panic=\"abort\"") && !compact.contains("panic='abort'"));
}
