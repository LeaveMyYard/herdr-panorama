//! What the action keys do (Python `SwarmApp.action_enter`, `action_answer`, `action_inbox`,
//! `action_transcript`, `action_filter`, `action_right`, `action_left`), for any view: each
//! returns the [`Effect`]s its actor carries out. What a view decides itself is its
//! [`Host`]'s.

use super::effect::{Effect, Toast};
use super::host::Host;
use crate::ui::graph::toggle;
use panorama_core::model::{HerdrStatus, Status};
use panorama_core::view::state::{DONE_CARD, FIN_CARD};
use panorama_core::view::{Filter, NEW_ROW};
use std::time::Instant;

/// The keys (Python `HELP`).
pub const HELP: &str = "↑↓/jk move · →/l/enter open · ←/h back · g go to pane · t transcript path · \
a answer (inbox) · i inbox · f filter · q/esc close · mouse: click a card to select it, again to go \
to its pane; wheel scrolls a column, shift+wheel scrolls the columns sideways";

/// The action keys, for every [`Host`].
pub trait Actions: Host {
    /// Enter: + New agent; an orchestrator, where Enter goes to panes: into its agent (with
    /// the arrows switching the chats, Enter is the way in); a done or finished card: unfold
    /// it; else open the next column.
    fn enter(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.armed(now) {
            return out;
        }
        let (focus, orch) = (self.view.focus, self.view.orch.clone());
        let thread = self.view.thread.clone().unwrap_or_default();
        let agent = self.view.agent.clone().unwrap_or_default();
        if focus == 0 && orch.as_deref() == Some(NEW_ROW) {
            return self.new_agent();
        } else if focus == 0 && self.enter_goes_to_pane() && orch.is_some() {
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

    /// a: answer the selection's questions in herdr-inbox (a blocked one: in its pane).
    fn answer(&mut self, now: Instant) -> Vec<Effect> {
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
    fn inbox(&mut self, now: Instant) -> Vec<Effect> {
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
    fn transcript(&self, now: Instant) -> Vec<Effect> {
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

    /// f: the next filter (the view's [`Host::set_filter`] shows it, and shares it).
    fn next_filter(&mut self, now: Instant) -> Vec<Effect> {
        let i = Filter::ALL
            .iter()
            .position(|f| *f == self.view.filter)
            .unwrap_or(0);
        self.set_filter(Filter::ALL[(i + 1) % Filter::ALL.len()], now)
    }

    /// →: into the next column; the docked list has one, so → does nothing there.
    fn right(&mut self) {
        if !self.dock() {
            self.step_right();
        }
    }

    /// ←: back a column; from a spawned agent, up to the agent that spawned it.
    fn left(&mut self) {
        self.step_left();
    }
}

impl<T: Host + ?Sized> Actions for T {}

/// `s` cut to `n` characters with an ellipsis (Python `herdr.clean(s, n)` on clean text).
fn truncated(s: &str, n: usize) -> String {
    herdr_agent_kit::herdr_api::clean(s, n)
}
