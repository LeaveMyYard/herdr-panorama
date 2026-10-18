//! An entry's state in the docked list (swarm-dock.js `oState`, turn 8's swarm-states.js;
//! Python `render.dock_state`): done > failed > blocked > working > needs you > waiting >
//! stale | idle.

use crate::model::{Agent, HerdrStatus, Orchestrator, Status, Thread, ThreadSource};
use crate::view::anim::{clock, glyph, spin};
use crate::view::colour::Rgb;
use crate::view::orch_state::Ages;
use crate::view::palette::{DIM, status};
use crate::view::text::ago;

/// One orchestrator as its docked entry shows it.
#[derive(Debug)]
pub struct DockState<'a> {
    pub st: Status,
    pub label: String,
    /// Its open questions (the session's, else its agents').
    pub q: usize,
    pub glyph: String,
    pub working: bool,
    /// Those still going.
    pub live: usize,
    /// Its threads not done, and those done.
    pub active: Vec<&'a Thread>,
    pub done: Vec<&'a Thread>,
    /// Background tasks in flight (the session's and its waiting agents').
    pub bg: usize,
}

/// The words for each state (`DOCK_LABEL`; questions read "needs you", as in the graph).
fn label(st: &Status) -> &'static str {
    match st {
        Status::Done => "done",
        Status::Failed => "failed",
        Status::Blocked => "blocked",
        Status::Running | Status::Reviewing => "working",
        Status::Waiting => "needs you",
        Status::Background => "waiting",
        Status::Stale => "stale",
        _ => "idle",
    }
}

/// The threads an entry lists: not the session's own.
fn threads(o: &Orchestrator) -> impl Iterator<Item = &Thread> {
    o.threads
        .iter()
        .filter(|t| t.source != ThreadSource::Session)
}

/// The entry's state. herdr reporting the session working counts as working, blocked as
/// blocked, and the session's open questions are its `?N`.
pub fn dock_state<'a>(o: &'a Orchestrator, frame: f64, ages: Ages) -> DockState<'a> {
    let agents: Vec<&Agent> = threads(o)
        .flat_map(|t| &t.agents)
        .filter(|a| !a.id.starts_with("main:"))
        .collect();
    let has = |st: Status| agents.iter().any(|a| a.status == st);
    let runner = agents.iter().copied().find(|a| a.status == Status::Running);
    let q = if o.inbox.is_empty() {
        agents.iter().map(|a| a.question_ids.len()).sum()
    } else {
        o.inbox.len()
    };
    let herdr_working = o.herdr_status == HerdrStatus::Working;
    let working = runner.is_some() || has(Status::Reviewing) || herdr_working;
    let bg = o.bg.len()
        + agents
            .iter()
            .filter(|a| a.status == Status::Background)
            .count();
    let (active, done): (Vec<&Thread>, Vec<&Thread>) = threads(o).partition(|t| !t.is_done());
    let stale = ages.is_stale(o);
    let blocked = has(Status::Blocked) || o.herdr_status == HerdrStatus::Blocked;
    let all_done = active.is_empty() && !done.is_empty();
    let mut words = None;
    let st = if all_done
        && !working
        && o.herdr_status != HerdrStatus::Blocked
        && q == 0
        && bg == 0
        && !stale
    {
        Status::Done
    } else if has(Status::Failed) {
        Status::Failed
    } else if blocked {
        Status::Blocked
    } else if working {
        if runner.is_some() || herdr_working {
            Status::Running
        } else {
            Status::Reviewing
        }
    } else if q > 0 {
        Status::Waiting
    } else if bg > 0 {
        let oldest = o.bg.iter().filter_map(|b| b.started).reduce(f64::min);
        if let Some(oldest) = oldest.filter(|t| ages.now - t > ages.stale_after) {
            words = Some(format!("waiting {}", ago(ages.now - oldest)));
        }
        Status::Background
    } else if stale {
        let since = o.last_activity.unwrap_or(ages.now);
        words = Some(format!("stale {}", ago(ages.now - since)));
        Status::Stale
    } else {
        Status::Queued
    };
    let g = match st {
        Status::Running => runner.map_or_else(|| spin(frame, 0).to_string(), |a| glyph(a, frame)),
        Status::Background => clock(frame, 0).to_string(),
        _ => st.glyph().to_owned(),
    };
    DockState {
        label: words.unwrap_or_else(|| label(&st).to_owned()),
        live: agents.iter().filter(|a| a.status.is_live()).count(),
        st,
        q,
        glyph: g,
        working,

        active,
        done,
        bg,
    }
}

/// The colour of the entry's state words (`_dock_stcol`).
pub fn state_colour(d: &DockState) -> Rgb {
    if d.st.needs_operator() {
        status(&d.st)
    } else if d.working {
        status(&Status::Running)
    } else if d.st == Status::Background {
        status(&Status::Background)
    } else {
        DIM
    }
}

/// A thread's glyph and the status it shows (swarm-dock.js `tState`): done, else its most
/// urgent live agent, else idle.
pub fn thread_glyph(t: &Thread, frame: f64) -> (String, Status) {
    if t.is_done() {
        return ("✓".into(), Status::Done);
    }
    match t
        .agents
        .iter()
        .filter(|a| a.status.is_live())
        .min_by_key(|a| a.status.urgency())
    {
        Some(a) => (glyph(a, frame), a.status.clone()),
        None => ("○".into(), Status::Queued),
    }
}
