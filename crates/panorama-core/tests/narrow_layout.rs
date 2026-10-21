//! The narrow layout and the navigation it walks, on the demo and deep data: Python's
//! tests/test_render.py, tests/test_deep.py and tests/test_states.py cases for what P3 draws
//! (the wide graph's own cases come with it in P4).

mod support;

use panorama_core::model::Status;
use panorama_core::view::filter::visible_orchs;
use panorama_core::view::nav::nav;
use panorama_core::view::orch_state::{Ages, orch_state};
use panorama_core::view::state::{DONE_CARD, FIN_CARD, NEW_ROW};
use panorama_core::view::{Filter, View, render_narrow};
use serde_json::json;
use support::{NO_SESSIONS, fixture, wire_swarm};

fn demo_view() -> View {
    // test_render.py `view()`: the demo's first orchestrator, thread and agent
    View {
        orch: Some("w1:p1".into()),
        thread: Some("ref:#42".into()),
        agent: Some("a2f81c0".into()),
        now: fixture("inputs/swarm-demo.json")["now"].as_f64().unwrap(),
        ..View::default()
    }
}

fn demo() -> panorama_core::model::Swarm {
    wire_swarm("inputs/swarm-demo.json", None)
}

#[test]
fn the_narrow_layout_is_an_indented_list_with_only_the_selected_orchestrator_open() {
    // test_render.py::test_narrow_layout_is_an_indented_list_with_only_the_selected_orchestrator_open
    let v = View {
        orch: Some("w1:p2".into()),
        thread: Some("wf:wf_search".into()),
        agent: Some("j1a2b3c".into()),
        ..demo_view()
    };
    let s = render_narrow(&demo(), &v, 70, 45, None, NO_SESSIONS);
    let text = s.text();
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows.iter().all(|r| r.chars().count() <= 70));
    for want in [
        "✗  Billing API  failed · working",
        "Search refactor  working  workflow",
        "!  Docs site  blocked",
        "✓  Release notes  all done",
        "search-index-rebuild",
        "Build · 22m",
        "✓ Plan         09:20–09:41",
        "author j1a2b3c · sonnet",
        "now  Read(src/search/tokenize.rs)",
        "✓ done ✓? done? ⠋ run » rev ○ queue ? ask ! block ✗ fail ~ lost",
    ] {
        assert!(text.contains(want), "missing {want:?}\n{text}");
    }
    assert!(rows[2].contains("✗ ?1"), "{}", rows[2]);
    assert!(
        rows[5].starts_with(" │") && rows[7].starts_with(" ╰─┨"),
        "{}\n{}",
        rows[5],
        rows[7]
    );
    assert!(rows[44].starts_with(" ↑↓ move   → open   g pane   a answer   f filter   ? help"));
}

#[test]
fn nav_lists_identities_per_column() {
    // test_render.py::test_nav_lists_identities_per_column
    let sw = demo();
    let cols = nav(&sw, &demo_view());
    assert_eq!(cols[0], ["w1:p1", "w1:p2", "w1:p3", "w1:p4", NEW_ROW]); // the + row is always last
    assert_eq!(
        cols[1],
        [
            "ref:#42",
            "ref:#44",
            "ref:#48",
            &format!("{DONE_CARD}w1:p1")
        ]
    );
    // live by start time, then the folded card
    assert_eq!(
        cols[2],
        ["a1c3d90", "a2f81c0", &format!("{FIN_CARD}ref:#42")]
    );
    let mut v = demo_view();
    v.done_open.insert("w1:p1".into());
    v.fin_open.insert("ref:#42".into());
    let cols = nav(&sw, &v);
    assert_eq!(cols[1].last().unwrap(), "ref:#40");
    assert_eq!(
        cols[2][cols[2].len() - 3..],
        ["a3e1b22", "a4d0e11", "a5b7f02"]
    );
}

/// tests/design_deep.py: `NOW`, `TID`; the view with the cursor on `sel` (test_deep.py
/// `view_for`).
const DEEP_NOW: f64 = 1_800_000_000.0;

fn deep_view(sel: &str) -> View {
    let depth = match sel {
        "b1" => 2,
        _ => 4,
    };
    View {
        orch: Some("w1:p3".into()),
        thread: Some("x:deep".into()),
        agent: Some(sel.into()),
        now: DEEP_NOW,
        focus: 1 + depth,
        ..View::default()
    }
}

#[test]
fn nav_has_one_column_per_depth_in_screen_order() {
    // test_deep.py::test_nav_has_one_column_per_depth_in_screen_order
    let sw = wire_swarm("inputs/swarm-deep.json", None);
    let cols = nav(&sw, &deep_view("c1"));
    let fin = |id: &str| format!("{FIN_CARD}{id}");
    assert_eq!(cols[2], ["l1", "v1"]);
    assert_eq!(cols[3], ["r1".to_owned(), "b1".into(), fin("l1")]);
    assert_eq!(
        cols[4],
        [
            "r2".to_owned(),
            "f1".into(),
            fin("r1"),
            "s1".into(),
            "s2".into(),
            "s6".into(),
            "s7".into(),
            "s8".into(),
            fin("b1")
        ]
    );
    assert_eq!(cols[5], ["c1"]);
    assert_eq!(cols[6], ["p1".to_owned(), fin("c1")]);
    assert_eq!(cols.len(), 7);
    let mut v = deep_view("c1");
    v.fin_open.insert("b1".into());
    let col4 = &nav(&sw, &v)[4];
    assert_eq!(
        col4[col4.len() - 4..],
        [fin("b1"), "s3".into(), "s4".into(), "s5".into()]
    ); // unfolded after its card
}

#[test]
fn the_needs_filter_keeps_the_path_to_an_agent_that_needs_you() {
    // test_deep.py::test_needs_filter_keeps_the_path_to_an_agent_that_needs_you
    let sw = wire_swarm("inputs/swarm-deep.json", None);
    let v = View {
        filter: Filter::Needs,
        ..deep_view("c1")
    };
    let cols = nav(&sw, &v);
    assert_eq!(cols[2], ["l1"]);
    assert_eq!(cols[3], ["r1", "b1"]);
    assert!(cols[5].contains(&"c1".to_owned()));
    assert!(!cols[2].contains(&"v1".to_owned()) && !cols[4].contains(&"s1".to_owned()));
}

#[test]
fn a_waiting_deep_agents_status_reaches_the_orchestrator() {
    // test_deep.py::test_status_glyph_of_a_waiting_deep_agent_reaches_the_orchestrator
    let sw = wire_swarm("inputs/swarm-deep.json", None);
    let o = &sw.orchestrators[0];
    assert!(o.agents().any(|a| a.status == Status::Waiting));
    assert!(matches!(
        orch_state(o, 0.0, None).st,
        Status::Failed | Status::Waiting
    ));
}

// -- test_states.py's rules (turn 8: waiting and stale) ------------------------------------

const NOW: f64 = 1_800_000_000.0;
const AGES: Ages = Ages {
    now: NOW,
    stale_after: 12.0 * 3600.0,
};

fn orch(extra: serde_json::Value) -> panorama_core::model::Orchestrator {
    let mut v = json!({"pane_id": "p", "tab_id": "t", "tab_label": "o", "session_id": "s",
        "herdr_status": "idle"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn bg(started: f64) -> serde_json::Value {
    json!([{"id": "b", "kind": "Bash", "description": "watch", "started": started}])
}

#[test]
fn idle_turns_stale_after_stale_after() {
    let st = |hours: f64| {
        orch_state(
            &orch(json!({"last_activity": NOW - hours * 3600.0})),
            0.0,
            Some(AGES),
        )
    };
    assert_ne!(st(11.0).st, Status::Stale);
    let s = st(13.0);
    assert_eq!(
        (
            s.st.clone(),
            s.label.as_str(),
            s.quiet(),
            s.st.needs_operator()
        ),
        (Status::Stale, "stale 13h", true, false)
    );
    assert_eq!(st(50.0).label, "stale 2d");
}

#[test]
fn waiting_wins_over_stale_and_says_its_age_once_older_than_stale_after() {
    let young = orch(json!({"last_activity": NOW - 13.0 * 3600.0, "bg": bg(NOW - 600.0)}));
    assert_eq!(orch_state(&young, 0.0, Some(AGES)).label, "waiting");
    let old = orch(json!({"last_activity": NOW - 13.0 * 3600.0, "bg": bg(NOW - 15.0 * 3600.0)}));
    let s = orch_state(&old, 0.0, Some(AGES));
    assert_eq!(
        (s.st.clone(), s.label.as_str(), s.bg, s.st.needs_operator()),
        (Status::Background, "waiting 15h", 1, false)
    );
}

#[test]
fn working_and_attention_win_over_waiting() {
    let o = orch(json!({"bg": bg(NOW), "herdr_status": "working"}));
    assert_eq!(orch_state(&o, 0.0, Some(AGES)).st, Status::Running);
    let o = orch(json!({"bg": bg(NOW), "inbox": [3]}));
    assert_eq!(orch_state(&o, 0.0, Some(AGES)).st, Status::Waiting);
}

#[test]
fn all_done_threads_go_stale_too() {
    let t = json!([{"id": "t", "name": "t", "source": "turn", "status": "done",
        "agents": [{"id": "a", "status": "done"}]}]);
    let fresh = orch(json!({"threads": t, "last_activity": NOW - 60.0}));
    assert_eq!(orch_state(&fresh, 0.0, Some(AGES)).st, Status::Done);
    let old = orch(json!({"threads": t, "last_activity": NOW - 13.0 * 3600.0}));
    assert_eq!(orch_state(&old, 0.0, Some(AGES)).st, Status::Stale);
}

#[test]
fn needs_me_shows_neither_waiting_nor_stale() {
    let sw: panorama_core::model::Swarm = serde_json::from_value(json!({"orchestrators": [
        serde_json::to_value(orch(json!({"bg": bg(NOW)}))).unwrap(),
        serde_json::to_value(orch(json!({"last_activity": NOW - 20.0 * 3600.0}))).unwrap(),
    ], "stale_after": 12.0 * 3600.0}))
    .unwrap();
    let v = |f| View {
        filter: f,
        now: NOW,
        ..View::default()
    };
    assert!(visible_orchs(&sw, &v(Filter::Needs)).is_empty());
    assert_eq!(visible_orchs(&sw, &v(Filter::Running)).len(), 1);
}

#[test]
fn the_narrow_layouts_colours_glide_between_whole_frames() {
    // test_smooth.py::test_restyling_equals_a_full_render[narrow, demo narrow]: between whole
    // frames the blinks and pulses move colours, never glyphs (Rust renders each colour tick
    // in full, so this is the check those rows carried)
    let sw = demo();
    let at = |frame: f64| {
        let v = View {
            orch: Some("w1:p1".into()),
            focus: 1,
            frame,
            ..demo_view()
        };
        render_narrow(&sw, &v, 70, 45, None, NO_SESSIONS)
    };
    let (whole, mid) = (at(1.0), at(1.5));
    assert_eq!(whole.text(), mid.text(), "a fraction moved a glyph");
    let colours = (0..45)
        .flat_map(|y| (0..70).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            whole.cell(x, y).fg != mid.cell(x, y).fg || whole.cell(x, y).bg != mid.cell(x, y).bg
        })
        .count();
    assert!(colours > 0, "nothing glides between frames 1 and 1.5");
    assert_eq!(at(2.0).text(), at(2.999).text()); // glyphs step only at whole frames
}

#[test]
fn the_new_row_in_the_narrow_layout() {
    // test_tui.py::test_new_row_in_narrow_layout
    let v = View {
        orch: Some(NEW_ROW.into()),
        now: demo_view().now,
        ..View::default()
    };
    let text = render_narrow(&demo(), &v, 70, 45, None, NO_SESSIONS).text();
    assert!(text.contains(" +   New agent  n"), "{text}");
}
