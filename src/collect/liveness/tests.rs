use super::*;
use std::sync::{Arc, Mutex};

fn counting(answer: Option<Pid>) -> (PanePids, Arc<Mutex<usize>>) {
    let asked = Arc::new(Mutex::new(0));
    let a = asked.clone();
    let pids = PanePids::new(Box::new(move |_| {
        *a.lock().unwrap() += 1;
        answer
    }));
    (pids, asked)
}

#[test]
fn a_pane_without_a_claude_process_is_not_asked_again_at_once() {
    let (mut pids, asked) = counting(None);
    assert_eq!(pids.pid("w1:p1", 100.0), None);
    assert_eq!(pids.pid("w1:p1", 120.0), None);
    assert_eq!(*asked.lock().unwrap(), 1);
    assert_eq!(pids.pid("w1:p1", 131.0), None);
    assert_eq!(*asked.lock().unwrap(), 2);
}

#[test]
fn a_found_process_is_kept_while_its_start_token_matches() {
    let me = std::process::id();
    let (mut pids, asked) = counting(Some(me));
    assert_eq!(pids.pid("w1:p1", 0.0), Some(me));
    assert_eq!(pids.pid("w1:p1", 1.0), Some(me));
    assert_eq!(*asked.lock().unwrap(), 1);
}

#[test]
fn tasks_are_read_lazily_and_once_per_collect() {
    let (pids, asked) = counting(None);
    let pids = RefCell::new(pids);
    let live = PaneLiveness::new("w1:p1", 0.0, &pids);
    assert_eq!(*asked.lock().unwrap(), 0);
    assert_eq!(live.task_alive("t1"), None);
    assert_eq!(live.task_alive("t2"), None);
    assert_eq!(*asked.lock().unwrap(), 1);
}
