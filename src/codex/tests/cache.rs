use super::{calls, fixture};
use crate::codex::Observer;
use std::time::{Duration, Instant};
fn finish(observer: &mut Observer, id: &str) -> Option<crate::codex::Observation> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let value = observer.observe(id);
        if !observer.is_pending(id) {
            return value;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn cache_is_nonblocking_reuses_results_and_does_not_confuse_same_cwd() {
    let (dir, options) = fixture("slow");
    let mut observer = Observer::new(options);
    let start = Instant::now();
    assert!(observer.observe("root-b").is_none());
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(observer.is_pending("root-b"));
    let data = finish(&mut observer, "root-b").unwrap();
    let count = calls(dir.path()).len();
    for _ in 0..10 {
        assert_eq!(observer.observe("root-b").unwrap(), data);
    }
    assert_eq!(calls(dir.path()).len(), count);
    assert!(observer.observe("root-a").is_none());
    assert!(observer.is_pending("root-a"));
}
#[test]
fn unavailable_finishes_pending_and_backs_off() {
    let (dir, options) = fixture("malformed");
    let mut observer = Observer::new(options);
    assert!(finish(&mut observer, "root-a").is_none());
    assert!(!observer.is_pending("root-a"));
    let count = calls(dir.path()).len();
    for _ in 0..10 {
        assert!(observer.observe("root-a").is_none());
    }
    assert_eq!(calls(dir.path()).len(), count);
    assert!(observer.observe("../bad").is_none());
    assert!(!observer.is_pending("../bad"));
}
#[test]
fn dropping_observer_cancels_queued_work_with_bounded_inflight_lifetime() {
    let (dir, mut options) = fixture("hang");
    options.timeout = Duration::from_millis(500);
    let mut observer = Observer::new(options);
    observer.observe("root-a");
    observer.observe("root-b");
    let deadline = Instant::now() + Duration::from_secs(2);
    while calls(dir.path()).is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let start = Instant::now();
    drop(observer);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(calls(dir.path()).len(), 1);
    #[cfg(target_os = "linux")]
    {
        let pid = std::fs::read_to_string(dir.path().join("pid")).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
}
