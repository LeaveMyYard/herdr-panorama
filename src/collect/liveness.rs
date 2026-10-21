//! Whether a session's background work still runs (Python `Collector._bg_alive`,
//! `_live_tasks`, `herdr.claude_pid`): a background Bash is alive while a descendant of the
//! pane's Claude process holds its output file. The pane's Claude pid is asked of herdr once
//! and kept while its start token matches; when herdr cannot name it, it is not asked again
//! for [`NO_PID_RETRY`] seconds. The process tree is read at most once per collect and pane,
//! and only when a task is open.

use herdr_agent_kit::agent_procs::claude::{SystemLiveness, first_claude, live_tasks};
use herdr_agent_kit::agent_procs::{Liveness, Pid, ProcessIdentity, start_token};
use herdr_agent_kit::herdr_api::PaneId;
use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeSet, HashMap};

/// Seconds before herdr is asked again for a pane's Claude process.
pub const NO_PID_RETRY: f64 = 30.0;

/// Finds a pane's Claude Code process (herdr's `pane process-info`).
pub type PanePid = dyn FnMut(&str) -> Option<Pid> + Send;

/// The pane's foreground Claude Code process, by herdr's `pane.process_info`.
pub fn claude_pid(herdr: &crate::trace::BlockingHerdrClient, pane: &str) -> Option<Pid> {
    let info = herdr.process_info(&PaneId::new(pane)).ok()?;
    let procs = info.foreground_processes.unwrap_or_default();
    let list: Vec<(Pid, &str, &[String])> = procs
        .iter()
        .filter_map(|p| {
            Some((
                Pid::try_from(p.pid?).ok()?,
                p.name.as_deref().unwrap_or(""),
                p.argv.as_slice(),
            ))
        })
        .collect();
    first_claude(list)
}

/// The panes' Claude processes, remembered across collects.
pub struct PanePids {
    pids: HashMap<String, (Pid, Option<String>)>,
    no_pid: HashMap<String, f64>,
    lookup: Box<PanePid>,
}

impl PanePids {
    /// Pids found with `lookup`.
    pub fn new(lookup: Box<PanePid>) -> Self {
        Self {
            pids: HashMap::new(),
            no_pid: HashMap::new(),
            lookup,
        }
    }

    /// The pane's Claude process now, if it can be told.
    pub fn pid(&mut self, pane: &str, now: f64) -> Option<Pid> {
        if let Some(&(pid, ref start)) = self.pids.get(pane)
            && start_token(pid) == *start
        {
            return Some(pid);
        }
        if self
            .no_pid
            .get(pane)
            .is_some_and(|t| *t > now - NO_PID_RETRY)
        {
            return None; // herdr could not name it a moment ago
        }
        match (self.lookup)(pane) {
            Some(pid) => {
                self.pids.insert(pane.to_owned(), (pid, start_token(pid)));
                Some(pid)
            }
            None => {
                self.pids.remove(pane);
                self.no_pid.insert(pane.to_owned(), now);
                None
            }
        }
    }
}

/// One pane's liveness for one collect: hook-recorded processes from the process table, its
/// background tasks from its Claude process's descendants (asked for lazily).
pub struct PaneLiveness<'a> {
    pane: &'a str,
    now: f64,
    pids: &'a RefCell<PanePids>,
    tasks: OnceCell<Option<BTreeSet<String>>>,
    processes: SystemLiveness,
}

impl<'a> PaneLiveness<'a> {
    /// The liveness of `pane`'s session at `now`.
    pub fn new(pane: &'a str, now: f64, pids: &'a RefCell<PanePids>) -> Self {
        Self {
            pane,
            now,
            pids,
            tasks: OnceCell::new(),
            processes: SystemLiveness::new(None),
        }
    }
}

impl Liveness for PaneLiveness<'_> {
    fn process_alive(&self, id: &ProcessIdentity) -> Option<bool> {
        self.processes.process_alive(id)
    }

    fn task_alive(&self, task: &str) -> Option<bool> {
        self.tasks
            .get_or_init(|| {
                self.pids
                    .borrow_mut()
                    .pid(self.pane, self.now)
                    .and_then(live_tasks)
            })
            .as_ref()
            .map(|t| t.contains(task))
    }
}

#[cfg(test)]
mod tests;
