//! The mouse (Python `Grid.on_click`, `on_mouse_move`, the wheel, `SwarmApp.click_target`,
//! `hover`, `wheel`): a click acts on the identity drawn under it at the last paint, only if
//! that identity is still shown, never on a cell that changed in the last refresh or scroll.
//! Selection clicks only move the cursor; action clicks are armed like their keys.

use super::actions::toggle;
use super::effect::Effect;
use crate::panel::model::Panel;
use panorama_core::view::nav::{agent_depth, find_orch, find_thread};
use panorama_core::view::state::DONE_CARD;
use panorama_core::view::target::AgentRef;
use panorama_core::view::{NEW_ROW, Screen, Target};
use std::time::Instant;

/// Rows a wheel step scrolls.
pub const WHEEL_ROWS: i32 = 3;

impl Panel {
    /// A click at `(x, y)` on `screen` (the last one painted).
    pub fn click(&mut self, x: i32, y: i32, screen: &Screen, now: Instant) -> Vec<Effect> {
        if !self.input.clicks.ready((x, y), now) {
            return vec![];
        }
        match screen.target_at(x, y).cloned() {
            Some(t) => self.click_target(t, now),
            None => vec![], // empty space: nothing
        }
    }

    fn click_target(&mut self, target: Target, now: Instant) -> Vec<Effect> {
        match target {
            Target::Filter(f) => self.set_filter(f, now),
            Target::NewAgent => {
                if !self.armed(now) {
                    return vec![];
                }
                self.select(0, NEW_ROW);
                self.graph.view.focus = 0;
                self.new_agent()
            }
            Target::Orch(pane) => self.click_orch(&pane, false, now),
            Target::AnswerOrch(pane) => self.click_orch(&pane, true, now),
            Target::Done(pane) => {
                let card = format!("{DONE_CARD}{pane}");
                if self.armed(now) && self.shown(&pane, Some(&card), None) {
                    self.point(&pane, Some(&card), None);
                    toggle(&mut self.graph.view.done_open, &pane); // what Enter does on it
                }
                vec![]
            }
            Target::Thread { pane, thread } => {
                if self.shown(&pane, Some(&thread), None) {
                    self.point(&pane, Some(&thread), None);
                }
                vec![]
            }
            Target::Agent(r) => self.click_agent(r, now),
            Target::Goto(r) => self.click_action(ActionKind::Goto, r, now),
            Target::Transcript(r) => self.click_action(ActionKind::Transcript, r, now),
            Target::Answer(r) => self.click_action(ActionKind::Answer, r, now),
            // the wide graph's (the app wires it, P4 8a); no panel view draws one
            Target::Fin { .. } => vec![],
        }
    }

    /// An orchestrator: in the docked list one click switches to it (what Enter then does);
    /// in the narrow layout it selects, and a second click goes to its pane. Its `?N`
    /// answers it.
    fn click_orch(&mut self, pane: &str, answer: bool, now: Instant) -> Vec<Effect> {
        if !self.shown(pane, None, None) {
            return vec![];
        }
        let again = self.graph.view.orch.as_deref() == Some(pane) && self.graph.view.focus == 0;
        if answer {
            if !self.armed(now) {
                return vec![];
            }
            self.point(pane, None, None);
            return self.answer(now);
        }
        if self.input.dock {
            self.point(pane, None, None);
            return self.goto(true, now);
        }
        if again {
            return self.goto(false, now); // the selected card, clicked again: its pane (g)
        }
        self.point(pane, None, None);
        vec![]
    }

    /// An agent's card or pod: select it; the selected one, clicked again: its pane.
    fn click_agent(&mut self, r: AgentRef, now: Instant) -> Vec<Effect> {
        if !self.shown(&r.pane, Some(&r.thread), Some(&r.agent)) {
            return vec![];
        }
        let mut out = vec![];
        self.seen(vec![r.agent.clone()], &mut out); // a click on an unseen card: seen
        let t = find_thread(find_orch(&self.graph.swarm, Some(&r.pane)), Some(&r.thread));
        let selected = self.graph.view.orch.as_deref() == Some(&r.pane)
            && self.graph.view.thread.as_deref() == Some(&r.thread)
            && self.graph.view.agent.as_deref() == Some(&r.agent)
            && self.graph.view.focus == 1 + agent_depth(t, Some(&r.agent));
        if selected {
            out.extend(self.goto(false, now));
        } else {
            self.point(&r.pane, Some(&r.thread), Some(&r.agent));
        }
        out
    }

    /// An agent's `g`, `t` or `a` hint: armed like its key.
    fn click_action(&mut self, kind: ActionKind, r: AgentRef, now: Instant) -> Vec<Effect> {
        if !self.shown(&r.pane, Some(&r.thread), Some(&r.agent)) {
            return vec![];
        }
        let mut out = vec![];
        self.seen(vec![r.agent.clone()], &mut out);
        if !self.armed(now) {
            return out;
        }
        self.point(&r.pane, Some(&r.thread), Some(&r.agent));
        out.extend(match kind {
            ActionKind::Goto => self.goto(false, now),
            ActionKind::Transcript => self.transcript(now),
            ActionKind::Answer => self.answer(now),
        });
        out
    }


}

/// Which action hint was clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Goto,
    Transcript,
    Answer,
}

#[cfg(test)]
mod tests;
