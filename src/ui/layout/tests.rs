//! The docked panel's width rule.

use super::docked;
use panorama_core::model::Swarm;
use panorama_core::view::empty::WIDE;

#[test]
fn docked_and_narrower_than_the_wide_layout_once_painted() {
    let docked_swarm = Swarm {
        docked: Some("w1:p3".into()),
        ..Swarm::default()
    };
    assert!(!docked(&docked_swarm, 0)); // no width yet
    assert!(docked(&docked_swarm, 46));
    assert!(docked(&docked_swarm, WIDE - 1));
    assert!(!docked(&docked_swarm, WIDE));
    assert!(!docked(&Swarm::default(), 46)); // nothing docked
}
