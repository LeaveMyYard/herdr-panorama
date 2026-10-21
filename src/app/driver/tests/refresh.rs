//! A refresh's outcome reaching the app: the swarm with the producer's unseen marks, or why
//! it failed.

use super::*;
use crate::app::collect::Collected;
use panorama_core::view::Unseen;

#[test]
fn a_refresh_brings_its_unseen_marks_and_a_failed_one_keeps_the_model() {
    let (sw, now) = wire("design9");
    let mut r = Run::new((sw.clone(), now), "w1:p3", false, (100, 45));
    let marks: Marks = [("a1".to_owned(), Unseen::Done)].into();
    r.d.collected(Collected::Swarm(sw.clone(), marks.clone()), r.t0);
    assert_eq!(r.d.app.graph.view.unseen, marks);
    r.d.collected(Collected::Failed("herdr is gone".into()), r.t0);
    assert_eq!(r.d.app.stale(), Some("herdr is gone"));
    assert_eq!(r.d.app.graph.swarm, sw); // the last good model stays
    assert_eq!(r.d.app.graph.view.unseen, marks);
}
