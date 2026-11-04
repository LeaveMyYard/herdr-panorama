//! An orchestrator's state as the graph's cards and the filters read it (Python
//! `render.state_of`, `orch_state`, `is_stale`): attention > working > reviewing > waiting >
//! stale | idle, and done when everything is.

use super::anim::{clock, glyph, spin};
use super::state::View;
use super::text::ago;
use crate::model::{Agent, BgTask, HerdrStatus, Orchestrator, Status, Swarm, Thread, ThreadSource};

/// The clock the ages are judged by: now, and how long idle reads "stale".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ages {
    pub now: f64,
    pub stale_after: f64,
}

impl Ages {
    /// The view's time and the swarm's `stale_after`.
    pub fn of(view: &View, swarm: &Swarm) -> Self {
        Self {
            now: view.now,
            stale_after: swarm.stale_after,
        }
    }

    /// Idle longer than `stale_after`: nothing in its transcripts or its agents' for that long.
    pub fn is_stale(&self, o: &Orchestrator) -> bool {
        o.last_activity
            .is_some_and(|t| self.now - t > self.stale_after)
    }
}

/// A card's state: its status, glyph and words.
#[derive(Clone, Debug, PartialEq)]
pub struct OrchState {
    pub st: Status,
    pub glyph: String,
    pub label: String,
    pub working: bool,
    /// Waiting: the background tasks in flight.
    pub bg: usize,
}

impl OrchState {
    /// A quiet state: nothing to watch (Python `OrchState.quiet` / `QUIET`): queued, done,
    /// cancelled, inferred done, lost, stale.
    pub fn quiet(&self) -> bool {
        is_quiet(&self.st)
    }

    fn new(st: Status, glyph: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            st,
            glyph: glyph.into(),
            label: label.into(),
            working: false,
            bg: 0,
        }
    }
}

/// Whether `st` is one of the quiet states (Python `QUIET`).
pub fn is_quiet(st: &Status) -> bool {
    matches!(
        st,
        Status::Queued
            | Status::Done
            | Status::Cancelled
            | Status::Inferred
            | Status::Lost
            | Status::Stale
    )
}

/// What the session itself adds to its agents' state (herdr's word, its questions, its own
/// background work, its idle age).
#[derive(Clone, Debug, Default)]
pub struct Extras<'a> {
    pub working: bool,
    pub blocked: bool,
    pub waiting: bool,
    pub bg: &'a [BgTask],
    pub stale_label: Option<String>,
}

/// The design's `stOf` over `agents` (Python `state_of`); `ages` `None`: no waiting age.
pub fn state_of(
    agents: &[&Agent],
    all_done: bool,
    frame: f64,
    x: &Extras,
    ages: Option<Ages>,
) -> OrchState {
    let has = |st: Status| agents.iter().any(|a| a.status == st);
    let runner = agents.iter().find(|a| a.status == Status::Running);
    let working = runner.is_some() || x.working;
    let waiting_agents: Vec<&&Agent> = agents
        .iter()
        .filter(|a| a.status == Status::Background)
        .collect();
    let n_bg = x.bg.len() + waiting_agents.len();
    if all_done && !working && !x.blocked && n_bg == 0 && x.stale_label.is_none() {
        return OrchState::new(Status::Done, "✓", "all done");
    }
    let attention = [
        (Status::Failed, "✗", "failed", has(Status::Failed)),
        (
            Status::Blocked,
            "!",
            "blocked",
            has(Status::Blocked) || x.blocked,
        ),
        (
            Status::Waiting,
            "?",
            "needs you",
            has(Status::Waiting) || x.waiting,
        ),
    ];
    if let Some((st, g, label, _)) = attention.into_iter().find(|a| a.3) {
        let label = if working {
            format!("{label} · working")
        } else {
            label.to_owned()
        };
        return OrchState {
            working,
            ..OrchState::new(st, g, label)
        };
    }
    if working {
        let g = runner.map_or_else(|| spin(frame, 0).to_string(), |a| glyph(a, frame));
        return OrchState {
            working: true,
            ..OrchState::new(Status::Running, g, "working")
        };
    }
    if has(Status::Reviewing) {
        return OrchState::new(Status::Reviewing, "»", "reviewing");
    }
    if n_bg > 0 {
        let tasks =
            x.bg.iter()
                .chain(waiting_agents.iter().flat_map(|a| a.bg.iter()));
        let oldest = tasks.filter_map(|b| b.started).reduce(f64::min);
        let mut label = "waiting".to_owned();
        if let (Some(ages), Some(oldest)) = (ages, oldest)
            && ages.now - oldest > ages.stale_after
        {
            label = format!("{label} {}", ago(ages.now - oldest));
        }
        return OrchState {
            bg: n_bg,
            ..OrchState::new(Status::Background, clock(frame, 0), label)
        };
    }
    if let Some(label) = &x.stale_label {
        return OrchState::new(Status::Stale, Status::Stale.glyph(), label.clone());
    }
    OrchState::new(Status::Queued, "○", "idle")
}

/// An orchestrator's card state (Python `orch_state`): herdr reporting the session working
/// counts as working, blocked as blocked, and its open questions as needs you. With `ages`:
/// waiting's age and stale (turn 8).
pub fn orch_state(o: &Orchestrator, frame: f64, ages: Option<Ages>) -> OrchState {
    let codex = o.harness == "codex";
    let stale_label = ages
        .filter(|_| !codex || o.herdr_status == HerdrStatus::Idle)
        .filter(|a| a.is_stale(o))
        .and_then(|a| o.last_activity.map(|t| format!("stale {}", ago(a.now - t))));
    // Codex's synthetic main agent mirrors herdr. Child/report attention is
    // meaningful, but neither its session card nor its history decides runtime.
    let agents: Vec<&Agent> = o
        .threads
        .iter()
        .flat_map(|thread| {
            thread
                .agents
                .iter()
                .filter(move |agent| !codex_root(o, thread, agent))
        })
        .collect();
    let extras = Extras {
        working: o.herdr_status == HerdrStatus::Working,
        blocked: o.herdr_status == HerdrStatus::Blocked,
        waiting: !o.inbox.is_empty()
            || (codex && agents.iter().any(|agent| !agent.question_ids.is_empty())),
        bg: &o.bg,
        stale_label,
    };
    let state = state_of(
        &agents,
        // Preserve the legacy Claude empty-session display. An unobserved harness
        // has no evidence of completed work merely because its thread list is empty.
        // A question no agent filed keeps the root needing you: the session's own card
        // carried it before the uniform tree (U3), the root itself does now.
        !codex
            && (o.harness == "claude" || !o.threads.is_empty())
            && o.threads.iter().all(|t| t.is_done())
            && o.inbox.is_empty(),
        frame,
        &extras,
        ages,
    );
    if codex && state.st == Status::Queued {
        match o.herdr_status {
            HerdrStatus::Done => return OrchState::new(Status::Done, "✓", "all done"),
            HerdrStatus::Unknown => {
                return OrchState::new(Status::Unknown("unknown".into()), "?", "unknown");
            }
            _ => {}
        }
    }
    state
}

/// Only the collector's exact associated session/root pair mirrors pane runtime.
/// A child or report may share its role, name or even a main-prefixed ID.
pub(super) fn codex_root(o: &Orchestrator, thread: &Thread, agent: &Agent) -> bool {
    o.harness == "codex"
        && thread.source == ThreadSource::Session
        && o.session_id
            .as_ref()
            .is_some_and(|id| thread.id == format!("codex:{id}"))
        && agent.id == format!("main:codex:{}", o.pane_id)
}

#[cfg(test)]
mod tests;
