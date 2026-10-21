//! A panel's filter reaching the home view through the core (Python
//! `tests/test_panel_filter.py::test_the_home_panorama_takes_a_panels_filter_and_publishes_it`).

use super::*;
use panorama_core::view::Filter;

#[test]
fn a_panels_filter_is_taken_and_a_bogus_one_ignored() {
    let mut r = Run::demo("w1:p1", (160, 45));
    let now = r.settled();
    assert_eq!(r.d.app.view.filter, Filter::All);
    assert!(
        r.d.app.filter_from_core("needs", now).is_some(),
        "taken: it goes out again"
    );
    assert_eq!(r.d.app.view.filter, Filter::Needs);
    assert_eq!(
        r.d.app.filter_from_core("needs", now),
        Some(vec![]),
        "already shown: still published"
    );
    assert_eq!(
        r.d.app.filter_from_core("bogus", now),
        None,
        "no such filter: ignored, nothing published"
    );
    assert_eq!(r.d.app.view.filter, Filter::Needs);
    assert!(r.d.app.filter_from_core("all", now).is_some());
    assert_eq!(r.d.app.view.filter, Filter::All);
}
