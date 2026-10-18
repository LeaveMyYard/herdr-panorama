//! What the action keys do (Python `SwarmApp.action_enter`, `action_goto`, `action_answer`,
//! `action_inbox`, `action_transcript`, `action_new_agent`, `action_filter`, `_seen`, with
//! `PanelApp`'s overrides): each returns the [`Effect`]s the actor carries out.

use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use panorama_core::model::{HerdrStatus, Status};
use panorama_core::view::NEW_ROW;
use panorama_core::view::nav::find_orch;
use panorama_core::view::state::{DONE_CARD, FIN_CARD};
use std::time::Instant;

/// The keys (Python `HELP`).
pub const HELP: &str = "↑↓/jk move · →/l/enter open · ←/h back · g go to pane · t transcript path · \
a answer (inbox) · i inbox · f filter · q/esc close · mouse: click a card to select it, again to go \
to its pane; wheel scrolls a column, shift+wheel scrolls the columns sideways";

impl Panel {
    /// Enter: + New agent; an orchestrator: into its agent (the arrows switch the chats, so
    /// Enter is the way in); a done or finished card: unfold it; else open the next column.
    pub fn enter(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.armed(now) {
            return out;
        }
        let (focus, orch) = (self.graph.view.focus, self.graph.view.orch.clone());
        let thread = self.graph.view.thread.clone().unwrap_or_default();
        let agent = self.graph.view.agent.clone().unwrap_or_default();
        if focus == 0 && orch.as_deref() == Some(NEW_ROW) {
            return self.new_agent();
        } else if focus == 0 && orch.is_some() {
            let switch = !self.arrows_mode();
            return self.goto(switch, now);
        } else if let Some(pane) = thread.strip_prefix(DONE_CARD).filter(|_| focus == 1) {
            toggle(&mut self.graph.view.done_open, pane);
        } else if let Some(key) = agent.strip_prefix(FIN_CARD).filter(|_| focus >= 2) {
            let fin = self.fin_agents(key);
            self.seen(fin, &mut out);
            toggle(&mut self.graph.view.fin_open, key);
        } else {
            if focus >= 2 && !agent.is_empty() {
                self.seen(vec![agent], &mut out);
            }
            self.right();
        }
        out
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
            && self.graph.swarm.docked.as_deref() != Some(o.pane_id.as_str())
            && self.input.settings.keep_focus;
        // its agents are seen once herdr confirms the pane is still there (the actor); while a
        // switch runs, that is all it does (`switch` makes no second one), as in Python
        out.push(Effect::Goto {
            pane: o.pane_id,
            label: o.tab_label,
            keep_focus,
            seen: ids,
        });
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
        let a = if self.graph.view.focus == 0 { None } else { a };
        let blocked = match a {
            Some(a) => a.status == Status::Blocked,
            None => o.herdr_status == HerdrStatus::Blocked,
        };
        if blocked {
            return self.goto(false, now); // a blocking prompt is answered in the pane itself
        }
        if !self.graph.swarm.inbox_ok {
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
        if self.usable(&mut out) && self.armed(now) && self.graph.swarm.inbox_ok {
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
        let o = find_orch(&self.graph.swarm, self.graph.view.orch.as_deref());
        let cwd = o
            .and_then(|o| o.cwd.clone())
            .or_else(|| self.graph.swarm.orchestrators.iter().find_map(|o| o.cwd.clone()));
        out.push(Effect::NewAgent { cwd });
        out
    }
}

pub(crate) use crate::ui::graph::toggle;

/// `s` cut to `n` characters with an ellipsis (Python `herdr.clean(s, n)` on clean text).
fn truncated(s: &str, n: usize) -> String {
    herdr_agent_kit::herdr_api::clean(s, n)
}

#[cfg(test)]
mod tests;
