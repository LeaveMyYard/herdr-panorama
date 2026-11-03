//! Exact app-home IPC status is liveness evidence; unknown is not absence.
use crate::host::command;
use std::{path::Path, process::Command, sync::atomic::AtomicBool, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Absent,
    Unknown,
}
pub fn probe(home: &Path, stop: &AtomicBool) -> State {
    let mut cmd = Command::new("herdr-app");
    cmd.arg("status")
        .env("HERDR_APP_HOME", home)
        .env_remove("HC_TERMINAL_ID");
    let Ok(output) = command::read(cmd, Duration::from_millis(500), 1024 * 1024, Some(stop)) else {
        return State::Unknown;
    };
    if output.status.success()
        && serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok_and(|v| v.is_object())
    {
        State::Running
    } else if output.status.code() == Some(2)
        && String::from_utf8_lossy(&output.stderr).starts_with("herdr-app: no Herdr window at ")
    {
        State::Absent
    } else {
        State::Unknown
    }
}
#[derive(Default)]
pub struct Idle {
    since: Option<Duration>,
}
impl Idle {
    pub fn observe(&mut self, state: State, now: Duration) -> bool {
        if state != State::Absent {
            self.since = None;
            return false;
        }
        now.saturating_sub(*self.since.get_or_insert(now)) >= Duration::from_secs(30)
    }
}

#[cfg(test)]
mod tests;
