//! The wide graph's columns and cards, each drawn on its own: the roots' column (depth 0)
//! and the uniform card at every depth (plan `uniform-agent-tree.md` §4).
//!
//! The design's and Python's columns are orchestrators and threads, so since the uniform tree
//! (U4, D5) the columns are compared with Rust's own reviewed renders,
//! `tests/fixtures/rust/screens/graph-columns.json` (`PANORAMA_BLESS=1`), and their rules are
//! checked on their own. The links between the columns are drawn by the page.

mod support;

use panorama_core::model::{Root, Swarm, TreeKids};
use panorama_core::view::graph::agent_card::{AgentCtx, agent_card};
use panorama_core::view::graph::column::{Column, Place};
use panorama_core::view::graph::orchs::orch_column;
use panorama_core::view::graph::root_card::loose_questions;
use panorama_core::view::nav::{default_children, find_agent, find_orch};
use panorama_core::view::{Filter, Screen, Target, View};
use serde_json::json;
use support::owned::check_owned;
use support::states::{NOW, states_swarm};
use support::{fixture, wire_swarm};

const FILE: &str = "graph-columns.json";

/// The first `h` rows of a column's tall canvas.
fn crop(canvas: &Screen, h: i32) -> Screen {
    let mut s = Screen::new(canvas.w(), h);
    s.blit(canvas, 0, 0, 0, h);
    s
}

/// The keyboard on the root `pane` (its card unfolded) or in its tree (`focus` > 0).
fn view_on(swarm: &Swarm, pane: &str, focus: usize, now: f64) -> View {
    let mut v = View {
        orch: Some(pane.into()),
        focus,
        now,
        ..View::default()
    };
    default_children(swarm, &mut v);
    v
}

/// `a`'s card drawn alone, 44 wide (`sel`: unfolded), with its root's tree.
fn agent_alone(o: &Root, id: &str, view: &View, sel: bool, kids_off: usize) -> Screen {
    let kids = TreeKids::of(o);
    let a = find_agent(Some(o), Some(id)).unwrap();
    let mut cv = Screen::new(44, 24);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: sel,
        focus: sel,
    };
    let ctx = AgentCtx {
        o,
        kids: &kids,
        inbox_ok: true,
        on_path: false,
        kids_off,
    };
    let h = agent_card(&mut cv, place, a, ctx, view);
    crop(&cv, h)
}

#[test]
fn turn_8_roots_column_the_selected_one_unfolded() {
    // design 8's roots: waiting, stale and blocked ones; the keyboard on Deploy Watch, which
    // has only its own background tasks: its card lists them
    let swarm = states_swarm(&fixture("design_8.json")["data"]);
    let view = view_on(&swarm, "w1:p5", 0, NOW);
    let col = orch_column(&swarm, &view, 42);
    let s = crop(&col.canvas, col.height);
    let text = s.text();
    assert!(text.contains("waiting · 2 bg"), "{text}");
    assert!(text.contains("g open pane"), "{text}");
    assert_eq!(col.anchors.len(), 8); // a card a root: its link joins its first line
    check_owned(FILE, "turn 8 roots", &s);
    // the running filter drops the stale roots
    let running = View {
        filter: Filter::Running,
        ..view.clone()
    };
    assert_eq!(orch_column(&swarm, &running, 42).anchors.len(), 6);
}

#[test]
fn the_demos_roots_column_fits_and_links_from_the_selected_card() {
    let swarm = wire_swarm("inputs/swarm-demo.json", None);
    let now = fixture("inputs/swarm-demo.json")["now"].as_f64().unwrap();
    let view = view_on(&swarm, "w1:p1", 1, now); // the keyboard in the tree: folded
    let col: Column = orch_column(&swarm, &view, 42);
    assert!(col.height <= 38, "the column fits: not scrolled");
    assert_eq!((col.sel, col.sel_anchor), (Some((0, 4)), Some(1)));
    check_owned(FILE, "demo roots", &crop(&col.canvas, col.height));
}

#[test]
fn a_roots_own_questions_and_prompt_are_in_its_unfolded_card() {
    // the interim U3 lost the "waiting on you" card of a root's own questions: the root card
    // carries them, with the prompt it blocks on and its answer hint
    let swarm = wire_swarm("rust/inputs/swarm-uniform.json", None);
    let now = fixture("rust/inputs/swarm-uniform.json")["now"]
        .as_f64()
        .unwrap();
    let o = find_orch(&swarm, Some("w1:p1")).unwrap();
    assert_eq!(
        loose_questions(o),
        ["Merge PR #41 with a merge commit or squash?"]
    );
    let view = view_on(&swarm, "w1:p1", 0, now);
    let col = orch_column(&swarm, &view, 42);
    let s = crop(&col.canvas, col.height);
    let text = s.text();
    // its hidden finished agents are counted in its own fold (`+2 older`), not twice
    assert!(
        text.contains("? Merge PR #41 with a merge commit") && !text.contains("older finished"),
        "{text}"
    );
    for t in [
        Target::GotoOrch("w1:p1".into()),
        Target::TranscriptOrch("w1:p1".into()),
        Target::AnswerOrch("w1:p1".into()),
    ] {
        assert!(s.targets.iter().any(|h| h.target == t), "{t:?}\n{text}");
    }
    check_owned(FILE, "uniform roots, Billing API open", &s);
    let view = view_on(&swarm, "w1:p3", 0, now);
    let col = orch_column(&swarm, &view, 42);
    let s = crop(&col.canvas, col.height);
    let text = s.text();
    assert!(
        text.contains("! Publish the v2 docs now or after")
            && text.contains("mkdocs serve")
            && text.contains("answer"),
        "{text}"
    );
    check_owned(FILE, "uniform roots, Docs site open", &s);
    // a root with no fold of its own says what the window hid in its card
    let view = view_on(&swarm, "w1:p4", 0, now);
    let col = orch_column(&swarm, &view, 42);
    let text = crop(&col.canvas, col.height).text();
    assert!(text.contains("+3 older finished, hidden"), "{text}");
}

#[test]
fn an_agent_card_carries_its_subtrees_marks_and_its_hidden_children() {
    let swarm = wire_swarm("rust/inputs/swarm-uniform.json", None);
    let now = fixture("rust/inputs/swarm-uniform.json")["now"]
        .as_f64()
        .unwrap();
    let o = find_orch(&swarm, Some("w1:p2")).unwrap();
    let view = view_on(&swarm, "w1:p2", 1, now);
    // folded, its children's column off to the right: `› n`, and the ✗ and ?1 below it
    let s = agent_alone(o, "s110", &view, false, 2);
    let row = s.text().lines().nth(1).unwrap().to_owned();
    assert!(row.contains("Trace the manifest cache"), "{row}");
    assert!(row.contains("✗ ?1 › 2"), "{row}");
    check_owned(FILE, "s110 folded", &s);
    // unfolded: its role, id and model on an `agent` line, its state on the right
    let s = agent_alone(o, "s112", &view, true, 0);
    let text = s.text();
    assert!(
        text.contains("agent s112 · gpt-5") && text.contains("? Bump Cargo.lock"),
        "{text}"
    );
    check_owned(FILE, "s112 open", &s);
}

#[test]
fn the_deep_trees_first_agent_folded_with_its_children_off_to_the_right() {
    let swarm = wire_swarm("inputs/swarm-deep.json", None);
    let o = find_orch(&swarm, Some("w1:p3")).unwrap();
    let kids = o
        .agents
        .iter()
        .filter(|a| a.parent.as_deref() == Some("l1"))
        .count();
    let view = view_on(&swarm, "w1:p3", 0, 1_800_000_000.0);
    let s = agent_alone(o, "l1", &view, false, kids);
    assert_eq!(s.h(), 3);
    assert!(s.text().contains(&format!("› {kids}")), "{}", s.text());
    check_owned(FILE, "deep l1 folded", &s);
}

#[test]
fn the_columns_rules_on_a_swarm_built_for_them() {
    // tests/fixtures/graph_rules.json: questions no agent filed (?2), a long name cut, the
    // root's own background task without a turn end, a waiting agent's oldest background
    // task (30m), a card on the selected agent's path
    let rules = fixture("graph_rules.json");
    let swarm: Swarm = serde_json::from_value(rules["swarm"].clone()).unwrap();
    let mut view = view_on(&swarm, "w1:p1", 0, rules["now"].as_f64().unwrap());
    view.unseen = serde_json::from_value(rules["unseen"].clone()).unwrap();
    let col = orch_column(&swarm, &view, 42);
    let s = crop(&col.canvas, col.height);
    let text = s.text();
    assert!(text.contains("?2") && text.contains('…'), "{text}");
    assert!(text.contains("Monitor") && text.contains("watch"), "{text}");
    check_owned(FILE, "rules roots", &s);
    let o = find_orch(&swarm, Some("w1:p1")).unwrap();
    view.focus = 1;
    view.agent = Some("n1".into());
    let s = agent_alone(o, "n1", &view, true, 0);
    assert!(s.text().contains("waiting 30m"), "{}", s.text());
    check_owned(FILE, "rules n1", &s);
    // a card on the selected agent's path: its title and border as selected
    let kids = TreeKids::of(o);
    let mut cv = Screen::new(44, 3);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: false,
        focus: false,
    };
    let ctx = AgentCtx {
        o,
        kids: &kids,
        inbox_ok: true,
        on_path: true,
        kids_off: 0,
    };
    let a0 = find_agent(Some(o), Some("a0")).unwrap();
    agent_card(&mut cv, place, a0, ctx, &view);
    check_owned(FILE, "rules a0 on the path", &cv);
}

#[test]
fn an_older_producers_threads_are_one_tree() {
    // a Python core's threads: every agent of every thread in one tree below its root
    let swarm: Swarm = serde_json::from_value(json!({"roots": [{
        "pane_id": "p", "tab_id": "t", "tab_label": "o", "session_id": "s",
        "agents": [{"id": "a", "status": "run"}, {"id": "b", "status": "run", "parent": "a"}, {"id": "c", "status": "run"}],
    }]}))
    .unwrap();
    let kids = TreeKids::of(&swarm.roots[0]);
    let ids = |p: Option<&str>| -> Vec<String> {
        kids.children(p).iter().map(|a| a.id.clone()).collect()
    };
    assert_eq!(ids(None), ["a", "c"]);
    assert_eq!(ids(Some("a")), ["b"]);
}
