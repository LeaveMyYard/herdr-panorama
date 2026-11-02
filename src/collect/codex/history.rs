//! Persisted Codex history in the existing display vocabulary. A completed turn is
//! a completed phase, never proof that its session or child agent has finished.

use crate::codex::{Thread, TurnStatus};
use crate::collect::py::chars;
use panorama_core::model::{Agent, HerdrStatus, Phase, PhaseState, Status, ThreadStatus};

pub(super) fn time(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

pub(super) fn runtime(status: &HerdrStatus) -> Status {
    match status {
        HerdrStatus::Working => Status::Running,
        HerdrStatus::Blocked => Status::Blocked,
        HerdrStatus::Idle => Status::Queued,
        HerdrStatus::Done => Status::Done,
        _ => Status::from_code("unknown"),
    }
}

pub(super) fn session_status(status: &HerdrStatus) -> ThreadStatus {
    match status {
        HerdrStatus::Done => ThreadStatus::Done,
        HerdrStatus::Working | HerdrStatus::Blocked | HerdrStatus::Idle => ThreadStatus::Active,
        _ => ThreadStatus::from_code("unknown"),
    }
}

/// Chronological phases. Only the newest, recorded in-progress turn can be
/// current, and only when herdr independently reports the root working/blocked.
pub(super) fn phases(thread: &Thread, runtime: &HerdrStatus) -> Vec<Phase> {
    thread
        .turns
        .iter()
        .enumerate()
        .rev()
        .map(|(index, turn)| Phase {
            name: chars(
                if turn.prompt.is_empty() {
                    &turn.id
                } else {
                    &turn.prompt
                },
                240,
            ),
            state: match turn.status {
                TurnStatus::Completed => PhaseState::Done,
                TurnStatus::Failed => PhaseState::Failed,
                TurnStatus::Interrupted => PhaseState::Skipped,
                TurnStatus::InProgress
                    if index == 0
                        && matches!(runtime, HerdrStatus::Working | HerdrStatus::Blocked) =>
                {
                    PhaseState::Now
                }
                _ => PhaseState::from_code("unknown"),
            },
            start: turn.started_at.and_then(time),
            end: turn.completed_at.and_then(time),
            note: (!turn.summary.is_empty()).then(|| chars(&turn.summary, 500)),
        })
        .collect()
}

/// Persisted turn outcomes do not establish whether a child is still alive.
/// Its most recent outcome is qualified as turn history in the message.
pub(super) fn agent(thread: &Thread, seq: i64) -> Agent {
    let latest = thread.turns.first();
    let tool = latest.and_then(|turn| {
        turn.latest_tool
            .as_ref()
            .filter(|tool| tool.turn_id == turn.id)
    });
    Agent {
        id: thread.thread_id.clone(),
        description: chars(
            thread
                .name
                .as_deref()
                .filter(|s| !s.is_empty())
                .or(thread.nickname.as_deref().filter(|s| !s.is_empty()))
                .unwrap_or(&thread.thread_id),
            240,
        ),
        role: chars(thread.role.as_deref().unwrap_or("agent"), 80),
        model: thread.model.as_deref().map(|s| chars(s, 120)),
        status: Status::from_code("unknown"),
        started: time(thread.created_at),
        last_at: time(thread.updated_at),
        tool_calls: latest.map_or(0, |turn| turn.tool_count.min(i64::MAX as usize) as i64),
        activity: tool.map_or_else(String::new, |tool| chars(&tool.name, 160)),
        last_message: latest.map_or_else(String::new, |turn| {
            let summary = chars(&turn.summary, 470);
            match turn.status {
                TurnStatus::Failed => format!("failed turn: {summary}"),
                TurnStatus::Interrupted => format!("interrupted turn: {summary}"),
                _ => chars(&turn.summary, 500),
            }
        }),
        seq,
        ..Agent::default()
    }
}
