//! The shared input over a stub view: what the panel's own tests cannot show (the panel always
//! goes to panes on Enter), and the default `seen`.

use super::*;
use crate::ui::graph::Graph;
use crate::ui::selection::initial;
use panorama_core::model::Swarm;
use panorama_core::view::state::{DONE_CARD, FIN_CARD};
use panorama_core::view::{Filter, NEW_ROW, Unseen};
use serde_json::Value;
use std::ops::{Deref, DerefMut};
use std::time::Instant;

/// A view with nothing of its own: armed unless a test says otherwise, never docked; it
/// records what it was asked.
struct Stub {
    graph: Graph,
    home: bool,
    /// Keys act (outside the arming window).
    armed: bool,
    gone_to: Vec<bool>,
}

impl Deref for Stub {
    type Target = Graph;
    fn deref(&self) -> &Graph {
        &self.graph
    }
}

impl DerefMut for Stub {
    fn deref_mut(&mut self) -> &mut Graph {
        &mut self.graph
    }
}

impl Host for Stub {
    fn armed(&self, _: Instant) -> bool {
        self.armed
    }
    fn click_ready(&self, _: (i32, i32), _: Instant) -> bool {
        true
    }
    fn dock(&self) -> bool {
        false
    }
    fn enter_goes_to_pane(&self) -> bool {
        self.home
    }
    fn goto(&mut self, switch: bool, _: Instant) -> Vec<Effect> {
        self.gone_to.push(switch);
        vec![]
    }
    fn new_agent(&mut self) -> Vec<Effect> {
        vec![]
    }
    fn set_filter(&mut self, f: Filter, _: Instant) -> Vec<Effect> {
        vec![Effect::SendFilter(f)]
    }
}

fn stub(home: bool) -> Stub {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-demo.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let swarm: Swarm = serde_json::from_value(v["swarm"].clone()).unwrap();
    let mut graph = Graph {
        swarm,
        ..Graph::default()
    };
    initial(&graph.swarm, &mut graph.view, Some("w1:p1"));
    Stub {
        graph,
        home,
        armed: true,
        gone_to: vec![],
    }
}

#[test]
fn enter_on_an_orchestrator_goes_to_its_pane_only_where_the_view_says_so() {
    // a popup (Python `SwarmApp` without home mode): Enter opens the threads
    let mut popup = stub(false);
    assert!(popup.enter(Instant::now()).is_empty());
    assert!(popup.gone_to.is_empty() && popup.view.focus == 1);
    // the home view: Enter goes to the pane
    let mut home = stub(true);
    home.enter(Instant::now());
    assert_eq!(home.gone_to, [true]); // not in the arrows' mode: the switch keeps the focus
    assert_eq!(home.view.focus, 0);
}

#[test]
fn seen_clears_the_marks_here_and_reports_them() {
    let mut s = stub(false);
    s.view.unseen.insert("a1".into(), Unseen::Fail);
    s.view.unseen.insert("b1".into(), Unseen::Done);
    let mut out = vec![];
    s.seen(vec!["a1".into()], &mut out);
    assert_eq!(out, [Effect::Seen(vec!["a1".into()])]);
    assert!(!s.view.unseen.contains_key("a1") && s.view.unseen.contains_key("b1"));
    let mut out = vec![];
    s.seen(vec![], &mut out);
    assert!(out.is_empty()); // nothing to report
}

#[test]
fn enter_inside_the_arming_window_does_nothing() {
    // Python arms the whole of `action_enter`: + New agent, a fin card, a done card alike
    let mut s = stub(true);
    s.armed = false;
    s.view.orch = Some(NEW_ROW.into());
    assert!(s.enter(Instant::now()).is_empty());
    let mut s = stub(false);
    s.armed = false;
    let fin = format!("{FIN_CARD}ref:#42");
    s.point("w1:p1", Some("ref:#42"), Some(&fin));
    assert!(s.enter(Instant::now()).is_empty() && s.view.fin_open.is_empty());
    s.armed = true;
    s.enter(Instant::now());
    assert!(s.view.fin_open.contains("ref:#42"));
}

#[test]
fn enter_on_a_done_card_unfolds_it_and_again_folds_it() {
    let mut s = stub(false);
    let card = format!("{DONE_CARD}w1:p1");
    s.point("w1:p1", Some(&card), None);
    s.enter(Instant::now());
    assert!(s.view.done_open.contains("w1:p1"));
    s.enter(Instant::now());
    assert!(!s.view.done_open.contains("w1:p1"));
}

#[test]
fn the_inbox_key_needs_the_inbox() {
    let mut s = stub(false);
    let now = Instant::now();
    assert_eq!(
        s.inbox(now),
        [Effect::OpenInbox {
            question: None,
            pane: None
        }]
    );
    s.swarm.inbox_ok = false;
    assert!(s.inbox(now).is_empty());
}

#[test]
fn answer_on_a_blocked_agent_goes_to_its_pane() {
    // the agent itself is blocked (its session not): a blocking prompt is answered in the pane
    let mut s = stub(false);
    s.swarm.orchestrators[2].herdr_status = panorama_core::model::HerdrStatus::Idle;
    s.point("w1:p3", Some("main:w1:p3"), Some("main:w1:p3"));
    assert_eq!(s.view.focus, 2);
    s.answer(Instant::now());
    assert_eq!(s.gone_to, [false]);
}
