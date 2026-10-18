use super::*;

fn line(s: &str) -> Arc<Vec<u8>> {
    Arc::new(s.as_bytes().to_vec())
}

fn drained(v: &Viewer) -> Vec<Arc<Vec<u8>>> {
    std::iter::from_fn(|| v.next()).collect()
}

#[test]
fn nothing_queued_after_shutdown_can_push_it_out() {
    // the review of #85 (N-3): open replies after shutdown filled the 4-deep outbox
    let v = Viewer::new(None, 4);
    v.send(line("state"));
    v.tell_to_go(line("shutdown"));
    for _ in 0..5 {
        v.send(line("opened"));
    }
    let got = drained(&v);
    assert_eq!(got.last(), Some(&line("shutdown")));
    assert!(!got.contains(&line("opened")));
}

#[test]
fn nothing_queued_after_a_refusal_can_push_it_out() {
    let v = Viewer::new(None, 4);
    v.finish(line("error"));
    for _ in 0..5 {
        v.send(line("ping"));
    }
    assert_eq!(drained(&v), [line("error")]);
    assert!(v.finishing());
}
