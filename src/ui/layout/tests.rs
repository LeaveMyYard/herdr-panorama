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
    assert!(!docked(&docked_swarm, 0, false)); // no width yet
    assert!(docked(&docked_swarm, 46, false));
    assert!(docked(&docked_swarm, WIDE - 1, false));
    assert!(!docked(&docked_swarm, WIDE, false));
    assert!(!docked(&Swarm::default(), 46, false)); // nothing docked
    // while a switch runs the docked panel stays, at any width (herdr may resize the pane)
    assert!(docked(&docked_swarm, WIDE + 40, true));
    assert!(!docked(&docked_swarm, 0, true) && !docked(&Swarm::default(), 46, true));
}
