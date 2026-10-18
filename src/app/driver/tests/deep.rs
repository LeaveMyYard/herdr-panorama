//! tests/test_deep.py's keys and pointer on the deep tree: the arrows walk it, → on a leaf or
//! a folded card does nothing, clicks focus their column, the wheel scrolls sideways.

use super::*;
use crate::ui::input::Clicks;
use panorama_core::view::state::FIN_CARD;
use panorama_core::view::target::AgentRef;

const TID: &str = "x:deep";

fn deep() -> Run {
    Run::new(wire("deep"), "w1:p3", false, (160, 45))
}

fn a(id: &str) -> AgentRef {
    AgentRef {
        pane: "w1:p3".into(),
        thread: TID.into(),
        agent: id.into(),
    }
}

#[test]
fn the_arrows_walk_the_tree_and_scroll_sideways() {
    let mut r = deep();
    let now = r.t0;
    let at = |r: &Run| {
        (
            r.d.app.view.focus,
            r.d.app.view.agent.clone().unwrap_or_default(),
        )
    };
    r.key(Key::Right, now);
    r.key(Key::Right, now);
    assert_eq!(
        (r.sel().0, r.sel().2, r.sel().3),
        (2, Some(TID), Some("l1"))
    );
    r.key(Key::Right, now);
    assert_eq!(at(&r), (3, "r1".into()));
    r.key(Key::Move(1), now);
    assert_eq!(at(&r).1, "b1");
    r.key(Key::Right, now);
    assert_eq!(at(&r), (4, "s1".into()));
    assert!(
        r.text()
            .contains("‹ Core Library › Refactor: plugin loader › lead l1   3 columns left")
    );
    r.key(Key::Left, now);
    assert_eq!(at(&r), (3, "b1".into())); // up to the agent that spawned it
    r.key(Key::Right, now);
    assert_eq!(at(&r).1, "s1"); // the child last selected there
    r.key(Key::Left, now);
    r.key(Key::Left, now);
    assert_eq!(at(&r), (2, "l1".into()));
    r.key(Key::Left, now);
    assert_eq!(at(&r), (1, "l1".into()));
    r.key(Key::Right, now);
    assert_eq!(at(&r), (2, "l1".into()));
}

#[test]
fn right_on_a_leaf_or_a_folded_card_does_nothing() {
    let mut r = deep();
    let now = r.settled();
    r.d.app.point("w1:p3", Some(TID), Some("v1"));
    r.key(Key::Right, now);
    assert_eq!(
        (r.d.app.view.focus, r.d.app.view.agent.as_deref()),
        (2, Some("v1"))
    );
    let fin = format!("{FIN_CARD}l1");
    r.d.app.point("w1:p3", Some(TID), Some(&fin));
    assert_eq!(r.d.app.view.focus, 3);
    r.key(Key::Enter, now);
    assert!(r.d.app.view.fin_open.contains("l1")); // Enter unfolds a nested group's finished agents
    r.key(Key::Right, now);
    assert_eq!(r.d.app.view.focus, 3);
}

#[test]
fn clicks_on_deep_cards_focus_their_column() {
    let mut r = deep();
    let now = r.settled();
    r.d.app.point("w1:p3", Some(TID), Some("r2"));
    assert_eq!(r.d.app.view.focus, 4);
    r.d.app.click_target(Target::Agent(a("f1")), now);
    assert_eq!(
        (r.d.app.view.focus, r.d.app.view.agent.as_deref()),
        (4, Some("f1"))
    );
    let fin = Target::Fin {
        pane: "w1:p3".into(),
        thread: TID.into(),
        group: Some("r1".into()),
    };
    r.d.app.click_target(fin, now);
    assert_eq!(
        r.d.app.view.agent.as_deref(),
        Some(format!("{FIN_CARD}r1").as_str())
    );
    assert!(r.d.app.view.fin_open.contains("r1"));
}

#[test]
fn shift_wheel_scrolls_sideways_and_a_focus_change_scrolls_back() {
    let mut r = deep();
    let now = r.settled();
    r.d.app.point("w1:p3", Some(TID), Some("b1"));
    r.paint(Painted::Other, now);
    assert_eq!(r.d.shown().unwrap().hlo, 2);
    let shown = r.d.shown().unwrap().clone();
    r.d.app.hwheel(2, &shown);
    r.paint(Painted::Wheel, now);
    assert_eq!(
        (r.d.app.view.hcol, r.d.app.view.agent.as_deref()),
        (Some(4), Some("b1"))
    ); // the focus stays
    assert!(r.text().contains(
        "‹ Core Library › Refactor: plugin loader › lead l1 › batch b1   4 columns left"
    ));
    let shown = r.d.shown().unwrap().clone();
    r.d.app.hwheel(-9, &shown);
    r.paint(Painted::Wheel, now);
    assert_eq!(r.d.app.view.hcol, Some(0));
    r.key(Key::Move(1), now);
    assert_eq!(r.d.app.view.hcol, None);
    assert_eq!(r.d.shown().unwrap().hlo, 2);
}

#[test]
fn shift_wheel_and_the_horizontal_wheel_scroll_sideways_over_a_column() {
    // test_deep.py::test_grid_maps_shift_wheel_and_the_horizontal_wheel_to_sideways
    let mut r = deep();
    let now = r.settled();
    r.d.app.point("w1:p3", Some(TID), Some("b1"));
    r.paint(Painted::Other, now);
    let (x, y) = (60, 10); // on b1's column
    let hcol = |r: &Run| r.d.app.view.hcol;
    assert_eq!(
        r.d.pointer(
            Pointer::Wheel {
                down: true,
                shift: true
            },
            x,
            y,
            now
        )
        .1,
        Some(Painted::Wheel)
    );
    assert_eq!(hcol(&r), Some(3));
    r.d.app.view.hcol = None;
    let (_, cause) = r.d.pointer(
        Pointer::Wheel {
            down: false,
            shift: false,
        },
        x,
        y,
        now,
    );
    assert_eq!((cause, hcol(&r)), (Some(Painted::Wheel), None)); // up: the column scrolls
    r.d.pointer(Pointer::Sideways { right: true }, x, y, now);
    assert_eq!(hcol(&r), Some(3));
    r.d.pointer(Pointer::Sideways { right: false }, x, y, now);
    assert_eq!(hcol(&r), Some(2));
    // not over a column: nothing
    assert_eq!(
        r.d.pointer(Pointer::Sideways { right: true }, 5, 0, now).1,
        None
    );
}

#[test]
fn a_click_right_after_a_sideways_scroll_does_not_hit_what_slid_under_the_pointer() {
    let mut r = deep();
    let now = r.settled();
    r.d.app.point("w1:p3", Some(TID), Some("b1"));
    r.paint(Painted::Other, now);
    let before = r.d.shown().unwrap().clone();
    r.d.app.hwheel(1, &before);
    r.paint(Painted::Wheel, now);
    let after = r.d.shown().unwrap().clone();
    let agent_at = |s: &Screen, x, y| match s.target_at(x, y) {
        Some(Target::Agent(a)) => Some(a.agent.clone()),
        _ => None,
    };
    let cells: Vec<(i32, i32)> = (0..after.h())
        .flat_map(|y| (0..after.w()).map(move |x| (x, y)))
        .collect();
    let moved = *cells
        .iter()
        .find(|&&(x, y)| {
            agent_at(&after, x, y).is_some() && before.target_at(x, y) != after.target_at(x, y)
        })
        .unwrap();
    let target = agent_at(&after, moved.0, moved.1).unwrap();
    r.click_at(moved, now + Duration::from_millis(100));
    assert_eq!(r.d.app.view.agent.as_deref(), Some("b1")); // ignored: it slid there
    if let Some(&same) = cells.iter().find(|&&(x, y)| {
        agent_at(&after, x, y).is_some() && before.target_at(x, y) == after.target_at(x, y)
    }) {
        r.click_at(same, now + Duration::from_millis(100));
        assert_eq!(r.d.app.view.agent, agent_at(&after, same.0, same.1));
    }
    r.click_at(moved, now + Duration::from_millis(400)); // the window passed
    assert_eq!(r.d.app.view.agent.as_deref(), Some(target.as_str()));
    assert!(crate::ui::input::arming::SCROLL_ARM < crate::ui::input::arming::CLICK_ARM);
}
