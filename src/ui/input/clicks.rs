//! Clicks (Python `SwarmApp.click_target` and its helpers), for any view: a click acts on
//! the identity drawn under it at the last paint, only if that identity is still shown, never
//! on a cell that changed in the last refresh or scroll. Selection clicks only move the
//! cursor; action clicks are armed like their keys.

use super::actions::Actions;
use super::effect::Effect;
use crate::ui::graph::toggle;
use panorama_core::view::nav::{agent_depth, find_orch};
use panorama_core::view::state::FIN_CARD;
use panorama_core::view::target::AgentRef;
use panorama_core::view::{NEW_ROW, Screen, Target};
use std::time::Instant;

/// Clicks, for every view with the [`Actions`].
pub trait Clicks: Actions {
    /// A click at `(x, y)` on `screen` (the last one painted).
    fn click(&mut self, x: i32, y: i32, screen: &Screen, now: Instant) -> Vec<Effect> {
        if !self.click_ready((x, y), now) {
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
                self.view.focus = 0;
                self.new_agent()
            }
            Target::Orch(pane) => self.click_orch(&pane, false, now),
            Target::AnswerOrch(pane) => self.click_orch(&pane, true, now),
            Target::GotoOrch(pane) => self.click_root_hint(&pane, false, now),
            Target::TranscriptOrch(pane) => self.click_root_hint(&pane, true, now),
            Target::Agent(r) => self.click_agent(r, now),
            Target::Goto(r) => self.click_action(ActionKind::Goto, r, now),
            Target::Transcript(r) => self.click_action(ActionKind::Transcript, r, now),
            Target::Answer(r) => self.click_action(ActionKind::Answer, r, now),
            // the wide graph's (the app wires it, P4 8a); no panel view draws one
            Target::Fin { pane, group } => self.click_fin(&pane, group.as_deref(), now),
        }
    }

    /// A root's `g` or `t` (`transcript`) hint: armed like its key.
    fn click_root_hint(&mut self, pane: &str, transcript: bool, now: Instant) -> Vec<Effect> {
        if !self.shown(pane, None) || !self.armed(now) {
            return vec![];
        }
        self.point(pane, None);
        if transcript {
            self.transcript(now)
        } else {
            self.goto(false, now)
        }
    }

    /// A group's "n finished" card (the wide graph's): it folds and unfolds like Enter on it,
    /// armed like Enter; its agents are seen.
    fn click_fin(&mut self, pane: &str, group: Option<&str>, now: Instant) -> Vec<Effect> {
        let key = group.unwrap_or(pane);
        let fid = format!("{FIN_CARD}{key}");
        let mut out = vec![];
        if !self.armed(now) || !self.shown(pane, Some(&fid)) {
            return out;
        }
        self.point(pane, Some(&fid));
        let fin = self.fin_agents(key);
        self.seen(fin, &mut out);
        toggle(&mut self.view.fin_open, key);
        out
    }

    /// A root: in the docked list one click switches to it (what Enter then does);
    /// in the narrow layout it selects, and a second click goes to its pane. Its `?N`
    /// answers it.
    fn click_orch(&mut self, pane: &str, answer: bool, now: Instant) -> Vec<Effect> {
        if !self.shown(pane, None) {
            return vec![];
        }
        let again = self.view.orch.as_deref() == Some(pane) && self.view.focus == 0;
        if answer {
            if !self.armed(now) {
                return vec![];
            }
            self.point(pane, None);
            return self.answer(now);
        }
        if self.dock() {
            self.point(pane, None);
            return self.goto(true, now);
        }
        if again {
            return self.goto(false, now); // the selected card, clicked again: its pane (g)
        }
        self.point(pane, None);
        vec![]
    }

    /// An agent's card or pod: select it; the selected one, clicked again: its pane.
    fn click_agent(&mut self, r: AgentRef, now: Instant) -> Vec<Effect> {
        if !self.shown(&r.pane, Some(&r.agent)) {
            return vec![];
        }
        let mut out = vec![];
        self.seen(vec![r.agent.clone()], &mut out); // a click on an unseen card: seen
        let o = find_orch(&self.swarm, Some(&r.pane));
        let selected = self.view.orch.as_deref() == Some(&r.pane)
            && self.view.agent.as_deref() == Some(&r.agent)
            && self.view.focus == agent_depth(o, Some(&r.agent));
        if selected {
            out.extend(self.goto(false, now));
        } else {
            self.point(&r.pane, Some(&r.agent));
        }
        out
    }

    /// An agent's `g`, `t` or `a` hint: armed like its key.
    fn click_action(&mut self, kind: ActionKind, r: AgentRef, now: Instant) -> Vec<Effect> {
        if !self.shown(&r.pane, Some(&r.agent)) {
            return vec![];
        }
        let mut out = vec![];
        self.seen(vec![r.agent.clone()], &mut out);
        if !self.armed(now) {
            return out;
        }
        self.point(&r.pane, Some(&r.agent));
        out.extend(match kind {
            ActionKind::Goto => self.goto(false, now),
            ActionKind::Transcript => self.transcript(now),
            ActionKind::Answer => self.answer(now),
        });
        out
    }
}

impl<T: Actions + ?Sized> Clicks for T {}

/// Which action hint was clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Goto,
    Transcript,
    Answer,
}
