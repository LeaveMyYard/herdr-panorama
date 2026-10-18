//! What the action keys do (Python `SwarmApp.action_enter`, `action_goto`, `action_answer`,
//! `action_inbox`, `action_transcript`, `action_new_agent`, `action_filter`, `_seen`, with
//! `PanelApp`'s overrides): each returns the [`Effect`]s the actor carries out.

use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use crate::panel::paint::NOTICE;
use panorama_core::model::{Agent, HerdrStatus, Orchestrator, Status, Thread};
use panorama_core::view::nav::{find_agent, find_orch, find_thread, tree_entries};
use panorama_core::view::state::{DONE_CARD, FIN_CARD};
use panorama_core::view::{Filter, NEW_ROW};
use std::time::Instant;

/// The keys (Python `HELP`).
pub const HELP: &str = "↑↓/jk move · →/l/enter open · ←/h back · g go to pane · t transcript path · \
a answer (inbox) · i inbox · f filter · q/esc close · mouse: click a card to select it, again to go \
to its pane; wheel scrolls a column, shift+wheel scrolls the columns sideways";

impl Panel {
    /// Keys act: nothing changed under them in the last [`super::ARM`].
    pub(crate) fn armed(&self, now: Instant) -> bool {
        now >= self.input.armed_at
    }

    /// Without a core the panel acts on nothing that needs one, and says so.
    fn usable(&self, out: &mut Vec<Effect>) -> bool {
        if !self.connected {
            out.push(Effect::Toast(Toast::new(None, NOTICE, 3)));
        }
        self.connected
    }

    /// The operator looked at these agents (the actor, after a confirmed switch): their marks
    /// clear here, and the core hears.
    pub fn seen_now(&mut self, ids: Vec<String>) -> Vec<Effect> {
        let mut out = vec![];
        self.seen(ids, &mut out);
        out
    }

    /// The operator looked at these agents: their marks clear here at once, and the core hears.
    pub(crate) fn seen(&mut self, ids: Vec<String>, out: &mut Vec<Effect>) {
        for id in &ids {
            self.view.unseen.remove(id);
        }
        if !ids.is_empty() {
            out.push(Effect::Seen(ids));
        }
    }

    /// What the action keys act on: (orchestrator, thread, agent); nothing while the focused
    /// column's cursor is parked, or on + New agent.
    pub(crate) fn selection(&self) -> (Option<&Orchestrator>, Option<&Thread>, Option<&Agent>) {
        if self.cur(self.view.focus).is_none() || self.view.orch.as_deref() == Some(NEW_ROW) {
            return (None, None, None);
        }
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let t = find_thread(o, self.view.thread.as_deref());
        let a = if self.view.focus >= 2 || t.is_some() {
            find_agent(t, self.view.agent.as_deref())
        } else {
            None
        };
        (o, t, a)
    }

    /// Enter: + New agent; an orchestrator: into its agent (the arrows switch the chats, so
    /// Enter is the way in); a done or finished card: unfold it; else open the next column.
    pub fn enter(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.armed(now) {
            return out;
        }
        let (focus, orch) = (self.view.focus, self.view.orch.clone());
        let thread = self.view.thread.clone().unwrap_or_default();
        let agent = self.view.agent.clone().unwrap_or_default();
        if focus == 0 && orch.as_deref() == Some(NEW_ROW) {
            return self.new_agent();
        } else if focus == 0 && orch.is_some() {
            let switch = !self.arrows_mode();
            return self.goto(switch, now);
        } else if let Some(pane) = thread.strip_prefix(DONE_CARD).filter(|_| focus == 1) {
            toggle(&mut self.view.done_open, pane);
        } else if let Some(key) = agent.strip_prefix(FIN_CARD).filter(|_| focus >= 2) {
            let fin = self.fin_agents(key);
            self.seen(fin, &mut out);
            toggle(&mut self.view.fin_open, key);
        } else {
            if focus >= 2 && !agent.is_empty() {
                self.seen(vec![agent], &mut out);
            }
            self.right();
        }
        out
    }

    /// The agents folded into a group's "n finished" card.
    fn fin_agents(&self, key: &str) -> Vec<String> {
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let Some(t) = find_thread(o, self.view.thread.as_deref()) else {
            return vec![];
        };
        let card = format!("{FIN_CARD}{key}");
        tree_entries(t, &self.view)
            .into_iter()
            .find(|e| e.ident == card)
            .map_or(vec![], |e| e.fin)
    }

    /// g (and Enter on an orchestrator): to the selected orchestrator's tab. `switch`: from
    /// the docked list onto another orchestrator, the keyboard stays on a panel there.
    pub fn goto(&mut self, switch: bool, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.usable(&mut out) || !self.armed(now) {
            return out;
        }
        self.input.arrow_due = None; // a key or click acts now: no settled-arrow switch after it
        let Some(o) = self.selection().0.cloned() else {
            return out;
        };
        let ids: Vec<String> = o.agents().map(|a| a.id.clone()).collect();
        if let Some(machine) = o.machine.clone() {
            self.seen(ids, &mut out);
            let machine_id = o.machine_id.clone().unwrap_or_else(|| machine.clone());
            out.push(Effect::GotoRemote {
                machine_id,
                machine,
                pane: o.pane_id,
                label: o.tab_label,
            });
            return out;
        }
        let keep_focus = switch
            && self.input.dock
            && self.swarm.docked.as_deref() != Some(o.pane_id.as_str())
            && self.input.settings.keep_focus;
        if !self.input.docking {
            // its agents are seen once herdr confirms the pane is still there (the actor)
            out.push(Effect::Goto {
                pane: o.pane_id,
                label: o.tab_label,
                keep_focus,
                seen: ids,
            });
        }
        out
    }

    /// a: answer the selection's questions in herdr-inbox (a blocked one: in its pane).
    pub fn answer(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.usable(&mut out) || !self.armed(now) {
            return out;
        }
        let (o, _, a) = self.selection();
        let Some(o) = o else { return out };
        if let Some(m) = &o.machine {
            let text = format!(
                "answering agents on {} is not supported yet",
                truncated(m, 60)
            );
            out.push(Effect::Toast(Toast::new(Some("another machine"), text, 6)));
            return out;
        }
        // on an orchestrator: its own state and questions, not a remembered agent's
        let a = if self.view.focus == 0 { None } else { a };
        let blocked = match a {
            Some(a) => a.status == Status::Blocked,
            None => o.herdr_status == HerdrStatus::Blocked,
        };
        if blocked {
            return self.goto(false, now); // a blocking prompt is answered in the pane itself
        }
        if !self.swarm.inbox_ok {
            return out;
        }
        if let Some(q) = a.and_then(|a| a.question_ids.iter().min().copied()) {
            out.push(Effect::OpenInbox {
                question: Some(q),
                pane: None,
            });
        } else if !o.inbox.is_empty() {
            out.push(Effect::OpenInbox {
                question: None,
                pane: Some(o.pane_id.clone()),
            });
        }
        out
    }

    /// i: herdr-inbox.
    pub fn inbox(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if self.usable(&mut out) && self.armed(now) && self.swarm.inbox_ok {
            out.push(Effect::OpenInbox {
                question: None,
                pane: None,
            });
        }
        out
    }

    /// t: the transcript's path, as a notice.
    pub fn transcript(&self, now: Instant) -> Vec<Effect> {
        if !self.armed(now) {
            return vec![];
        }
        let (o, _, a) = self.selection();
        let path = a
            .and_then(|a| a.transcript.clone())
            .or_else(|| o.and_then(|o| o.transcript.clone()));
        let text = truncated(path.as_deref().unwrap_or("no transcript"), 300);
        vec![Effect::Toast(Toast::new(Some("transcript"), text, 8))]
    }

    /// n: + New agent, in its own herdr popup, in the selected orchestrator's directory (else
    /// any orchestrator's; the popup's own default without one).
    pub fn new_agent(&mut self) -> Vec<Effect> {
        let mut out = vec![];
        if !self.usable(&mut out) {
            return out;
        }
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let cwd = o
            .and_then(|o| o.cwd.clone())
            .or_else(|| self.swarm.orchestrators.iter().find_map(|o| o.cwd.clone()));
        out.push(Effect::NewAgent { cwd });
        out
    }

    /// f: the next filter, shown here at once and sent to the core for every other view.
    pub fn next_filter(&mut self, now: Instant) -> Vec<Effect> {
        let i = Filter::ALL
            .iter()
            .position(|f| *f == self.view.filter)
            .unwrap_or(0);
        self.set_filter(Filter::ALL[(i + 1) % Filter::ALL.len()], now)
    }

    /// The operator chose `f` here (f, or a click on a filter): shown at once, and sent to the
    /// core for every other view; kept against the core's older word until it echoes.
    pub(crate) fn set_filter(&mut self, f: Filter, now: Instant) -> Vec<Effect> {
        self.apply_filter(f);
        self.input.filter_sent = Some((f, now));
        vec![Effect::SendFilter(f)]
    }

    /// The filter did not go out (no core): the next core's state decides.
    pub fn filter_not_sent(&mut self) {
        self.input.filter_sent = None;
    }
}

/// Adds `key` to `set`, or takes it out.
pub(crate) fn toggle(set: &mut std::collections::BTreeSet<String>, key: &str) {
    if !set.remove(key) {
        set.insert(key.to_owned());
    }
}

/// `s` cut to `n` characters with an ellipsis (Python `herdr.clean(s, n)` on clean text).
fn truncated(s: &str, n: usize) -> String {
    herdr_agent_kit::herdr_api::clean(s, n)
}

#[cfg(test)]
mod tests;
