//! What a view supplies to the shared input (Python: what `PanelApp` overrides in
//! `SwarmApp`): when its keys and clicks act, its docked list, where Enter on an orchestrator
//! leads, and the actions whose IO differs (going to a pane, a new agent, the filter).

use super::effect::Effect;
use crate::ui::graph::Graph;
use panorama_core::view::Filter;
use std::ops::DerefMut;
use std::time::Instant;

/// A view the shared input drives: its [`Graph`], and what it decides itself.
pub trait Host: DerefMut<Target = Graph> {
    /// Whether the keys act at `now`: nothing changed under them in the last arming window.
    fn armed(&self, now: Instant) -> bool;

    /// Whether a click on `cell` acts at `now` (the click arming).
    fn click_ready(&self, cell: (i32, i32), now: Instant) -> bool;

    /// The docked list: one column.
    fn dock(&self) -> bool;

    /// Enter on an orchestrator goes to its pane (the home view and the panels); in a popup
    /// it opens the next column.
    fn enter_goes_to_pane(&self) -> bool;

    /// The arrows switch the chats (the docked list): Enter then goes into the agent.
    fn arrows_mode(&self) -> bool {
        false
    }

    /// Whether the actions that need the view's link (a panel's core) act; if not, `out` says
    /// why.
    fn usable(&self, out: &mut Vec<Effect>) -> bool {
        let _ = out;
        true
    }

    /// g (and Enter on an orchestrator): to the selected orchestrator's pane; `switch`: from
    /// the docked list onto another orchestrator.
    fn goto(&mut self, switch: bool, now: Instant) -> Vec<Effect>;

    /// n: + New agent.
    fn new_agent(&mut self) -> Vec<Effect>;

    /// A filter chosen here (f, or a click on one).
    fn set_filter(&mut self, f: Filter, now: Instant) -> Vec<Effect>;

    /// The operator looked at these agents: their marks clear here at once, and the actor
    /// tells whoever keeps them ([`Effect::Seen`]).
    fn seen(&mut self, ids: Vec<String>, out: &mut Vec<Effect>) {
        for id in &ids {
            self.view.unseen.remove(id);
        }
        if !ids.is_empty() {
            out.push(Effect::Seen(ids));
        }
    }
}
