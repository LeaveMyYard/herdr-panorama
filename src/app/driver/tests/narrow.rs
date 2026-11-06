//! tests/test_mouse.py's narrow layout (a side split): clicks, the opened agent's actions, the
//! wheel and the + row; and tests/test_tui.py::test_narrow_layout_in_a_side_split.

use super::*;
use panorama_core::view::Filter;
use panorama_core::view::target::AgentRef;

const NARROW: (i32, i32) = (70, 45);

fn agent(a: &str) -> AgentRef {
    AgentRef {
        pane: "w1:p1".into(),
        agent: a.into(),
    }
}

#[test]
fn narrow_clicks() {
    let mut r = Run::demo("w1:p1", NARROW);
    let now = r.settled();
    // interim (plan `uniform-agent-tree.md` U4), until U5 replaces narrow: no thread
    // selection, an agent's pod selects it at its depth
    r.click(&Target::Agent(agent("b3a9c41")), now); // its pod
    assert_eq!(r.sel(), (1, Some("w1:p1"), Some("b3a9c41")));
    r.click(&orch("w1:p2"), now);
    assert_eq!((r.sel().0, r.sel().1), (0, Some("w1:p2")));
    r.click(&Target::Filter(Filter::Running), now);
    assert_eq!(r.d.app.view.filter, Filter::Running);
    let later = now + Duration::from_secs(2);
    assert!(goes_to(&r.click(&orch("w1:p2"), later), "w1:p2")); // selected and focused: its pane
}

#[test]
fn narrow_expanded_agent_and_its_actions() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, NARROW);
    let now = r.settled();
    let a = agent("a1c3d90");
    assert!(
        r.spot(&Target::Goto(a.clone())).is_some()
            && r.spot(&Target::Transcript(a.clone())).is_some()
    );
    r.click(&Target::Agent(a.clone()), now);
    assert_eq!(r.sel(), (1, Some("w1:p1"), Some("a1c3d90")));
    assert!(goes_to(&r.click(&Target::Goto(a), now), "w1:p1"));
}

#[test]
fn narrow_wheel_and_new_row() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, (70, 14));
    let now = r.settled();
    assert!(r.spot(&Target::NewAgent).is_none()); // below the fold
    for _ in 0..30 {
        if let (_, Some(cause)) = r.d.pointer(
            Pointer::Wheel {
                down: true,
                shift: false,
            },
            10,
            4,
            now,
        ) {
            r.paint(cause, now);
        }
    }
    let later = now + Duration::from_secs(2);
    let out = r.click(&Target::NewAgent, later);
    assert!(
        matches!(out.as_slice(), [Effect::NewAgent { .. }]),
        "{out:?}"
    );
}

#[test]
fn the_narrow_layout_in_a_side_split() {
    let mut r = Run::demo("w1:p1", NARROW);
    let text = r.text();
    assert!(
        text.contains("Billing API  failed · working") && text.contains("Search refactor  working")
    );
    // the selected root's subagents as cards, its finished ones folded
    assert!(text.contains("┨ » reviewer a2f81c0"), "{text}");
    assert!(text.contains("8 finished · author e1a2b3c"), "{text}");
    r.key(Key::Move(1), r.t0);
    let text = r.text();
    let row = text
        .lines()
        .find(|l| l.contains("Search refactor"))
        .unwrap();
    assert!(row.starts_with("▌ "), "{text}"); // the focus mark on Search refactor
}
