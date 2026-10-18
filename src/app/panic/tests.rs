//! The app's panic hook and the build that lets it contain panics.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
static RESTORED: AtomicUsize = AtomicUsize::new(0);

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
        Box::new(|_| {
            NEXT.fetch_add(1, Ordering::SeqCst);
        }),
    ));
    let (n0, r0) = (NEXT.load(Ordering::SeqCst), RESTORED.load(Ordering::SeqCst));
    // on the collect thread: contained
    let t = std::thread::Builder::new()
        .name(THREAD.into())
        .spawn(|| std::panic::catch_unwind(|| panic!("in a collect")).is_err())
        .unwrap();
    assert!(t.join().unwrap());
    let (n1, r1) = (NEXT.load(Ordering::SeqCst), RESTORED.load(Ordering::SeqCst));
    // anywhere else: the terminal comes back, then the default hook
    let t = std::thread::spawn(|| std::panic::catch_unwind(|| panic!("in the view")).is_err());
    assert!(t.join().unwrap());
    let (n2, r2) = (NEXT.load(Ordering::SeqCst), RESTORED.load(Ordering::SeqCst));
    std::panic::set_hook(prev);
    assert_eq!((n1 - n0, r1 - r0), (0, 0));
    assert_eq!((n2 - n1, r2 - r1), (1, 1));
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
