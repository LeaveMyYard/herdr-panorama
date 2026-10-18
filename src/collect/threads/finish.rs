//! A thread's status, times and order (Python `threads.thread_status`, `finish`,
//! `sort_threads`, `collect.flavour`).

use super::Draft;
use crate::collect::agents::Arena;
use crate::collect::py::{or_time, truthy_time};
use panorama_core::model::{Agent, Status, ThreadSource, ThreadStatus};

/// Active while an agent is live (a failed one is not); else failed when the newest agent
/// failed, cancelled when all were, done otherwise (and done when it has none).
pub fn thread_status(agents: &[&Agent]) -> ThreadStatus {
    if agents
        .iter()
        .any(|a| a.status.is_live() && a.status != Status::Failed)
    {
        return ThreadStatus::Active;
    }
    let key = |a: &Agent| or_time(a.ended, a.last_at).unwrap_or(0.0);
    let newest = agents.iter().fold(None::<&&Agent>, |best, a| match best {
        Some(b) if key(b) >= key(a) => Some(b),
        _ => Some(a),
    });
    match newest {
        None => ThreadStatus::Done,
        Some(n) if n.status == Status::Failed => ThreadStatus::Failed,
        Some(_) if agents.iter().all(|a| a.status == Status::Cancelled) => ThreadStatus::Cancelled,
        Some(_) => ThreadStatus::Done,
    }
}

/// Whether a status is still unset (Python's `""`).
pub fn unset(s: &ThreadStatus) -> bool {
    s.code().is_empty()
}

/// Fill the status (unless set), the start and the end; order the agents by urgency, then
/// start.
pub fn finish(d: &mut Draft, arena: &Arena) {
    let ags: Vec<&Agent> = d.ids.iter().map(|id| &arena[id]).collect();
    let t = &mut d.thread;
    if unset(&t.status) {
        t.status = thread_status(&ags);
    }
    let started = ags
        .iter()
        .filter_map(|a| truthy_time(a.started))
        .reduce(f64::min);
    t.started = or_time(t.started, started);
    if t.is_done() || t.status == ThreadStatus::Failed {
        let ended = ags
            .iter()
            .filter_map(|a| or_time(a.ended, a.last_at))
            .filter(|e| *e != 0.0)
            .reduce(f64::max);
        t.ended = or_time(t.ended, ended.or(t.started));
    }
    d.ids.sort_by(|a, b| {
        let (a, b) = (&arena[a], &arena[b]);
        a.status.urgency().cmp(&b.status.urgency()).then(
            a.started
                .unwrap_or(0.0)
                .total_cmp(&b.started.unwrap_or(0.0)),
        )
    });
}

/// Not done first; then by the most urgent agent (a failed thread first); open threads oldest
/// first, done ones newest first.
pub fn sort_threads(mut drafts: Vec<Draft>, arena: &Arena) -> Vec<Draft> {
    let key = |d: &Draft| {
        let t = &d.thread;
        let top = d
            .ids
            .iter()
            .map(|a| arena[a].status.urgency())
            .min()
            .unwrap_or(99);
        let second = if t.status == ThreadStatus::Failed {
            0
        } else {
            top
        };
        let started = t.started.unwrap_or(0.0);
        (
            t.is_done(),
            second,
            if t.is_done() { -started } else { started },
        )
    };
    drafts.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0)
            .then(ka.1.cmp(&kb.1))
            .then(ka.2.total_cmp(&kb.2))
    });
    drafts
}

/// The orchestrator's flavour: an explicit thread's, else "workflow", "pr-review" or
/// "agents" by the sources found.
pub fn flavour(drafts: &[Draft]) -> String {
    let threads = drafts.iter().map(|d| &d.thread);
    if let Some(f) = threads
        .clone()
        .filter(|t| t.source == ThreadSource::Explicit)
        .find_map(|t| t.flavour.as_deref().filter(|f| !f.is_empty()))
    {
        return f.to_owned();
    }
    let has = |s: ThreadSource| threads.clone().any(|t| t.source == s);
    if has(ThreadSource::Workflow) {
        "workflow".into()
    } else if has(ThreadSource::Ref) {
        "pr-review".into()
    } else {
        "agents".into()
    }
}
