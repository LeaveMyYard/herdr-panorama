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

#[test]
fn a_panels_seen_clears_the_view_and_reaches_the_tracker_and_its_file() {
    // a mark cleared by a click in a panel must not come back on the next refresh: Python
    // routes the core's `seen` through `_seen` (the tracker, then the file)
    use crate::unseen::{Event, Tracker};
    use std::sync::Arc;
    let (sw, now) = wire("design9");
    let agent = sw.orchestrators[0].threads[0].agents[0].id.clone();
    let mut r = Run::new((sw.clone(), now), "w1:p3", false, (100, 45));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unseen.json");
    let tracker = Arc::new(Tracker::new(path.clone(), Box::new(|| true)));
    let mut ended = sw.clone();
    let status = |s: &mut Swarm, code: &str| {
        s.orchestrators[0].threads[0].agents[0].status =
            panorama_core::model::Status::from_code(code);
    };
    status(&mut ended, "run");
    tracker.update(&ended, 1.0);
    status(&mut ended, "done");
    assert_eq!(tracker.update(&ended, 2.0).get(&agent), Some(&Event::Done));
    r.d.collected(
        Collected::Swarm(ended.clone(), [(agent.clone(), Unseen::Done)].into()),
        r.t0,
    );
    let out = r.d.seen_in_a_panel(vec![agent.clone()]);
    assert!(!r.d.app.graph.view.unseen.contains_key(&agent)); // cleared here at once
    assert_eq!(out, [Effect::Seen(vec![agent.clone()])]); // and the tracker hears it
    for e in out {
        if let Effect::Seen(ids) = e {
            crate::app::seen::mark(&tracker, &ids)
                .unwrap()
                .join()
                .unwrap();
        }
    }
    // the next refresh, here or in any panorama reading the file, keeps it seen
    assert!(!tracker.update(&ended, 3.0).contains_key(&agent));
    let other = Tracker::new(path, Box::new(|| true));
    assert!(!other.update(&ended, 4.0).contains_key(&agent));
}
