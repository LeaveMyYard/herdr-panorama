//! Read only recorded addresses; cwd, labels and recency never select a session.
use super::{Core, context::pane_key, core::owns, link::Signal};
use herdr_agent_kit::agent_procs::{ProcessTable, SystemProcesses};
use serde_json::Value;
use std::{collections::HashSet, path::Path};

mod load;

pub use load::{Hooks, load};

#[derive(Clone)]
pub struct Record {
    pub session: String,
    pub terminal: String,
    pub harness: String,
    pub pid: u32,
    pub start: String,
    pub pidns: Option<String>,
    /// The hook's clock; NaN when missing. Orders one process's records only (§3 H).
    pub ts: f64,
}

/// Evidence H (session-linking.md §3): the sessions named by root SessionStart records of the
/// terminal's current Claude process. Several are ambiguous; the newest is kept only so that
/// Claude's own per-process record can corroborate it. A hook file that was not read in full may
/// hold another of this process's sessions, so what was read is treated the same way.
pub fn claude_session(
    hooks: &Hooks,
    address: &str,
    info: &Value,
    table: &dyn ProcessTable,
) -> Signal {
    let mut ids = HashSet::new();
    let mut newest: Option<&Record> = None;
    let (mut ordered, mut tie) = (true, false);
    for r in hooks
        .records
        .iter()
        .filter(|r| r.terminal == address && r.harness == "claude")
    {
        if proves_process(r, address, info, table) {
            ids.insert(&r.session);
            ordered &= !r.ts.is_nan();
            match newest {
                Some(n) if r.ts == n.ts => tie |= r.session != n.session,
                Some(n) if r.ts < n.ts => {}
                _ => (newest, tie) = (Some(r), false),
            }
        }
    }
    match (ids.len(), newest) {
        (_, None) => Signal::Absent,
        (1, Some(r)) if hooks.complete => Signal::Names(r.session.clone()),
        (_, Some(r)) if ordered && !tie => Signal::Newest(r.session.clone()),
        _ => Signal::Ambiguous,
    }
}

/// The record's process is the terminal's current foreground Claude.
fn proves_process(r: &Record, address: &str, info: &Value, table: &dyn ProcessTable) -> bool {
    let argv = table.cmdline(r.pid);
    let name = table.stat(r.pid).map_or(String::new(), |p| p.comm);
    herdr_agent_kit::agent_procs::claude::first_claude([(r.pid, name.as_str(), argv.as_slice())])
        .is_some()
        && namespace_matches(r.pidns.as_deref())
        && owns(info, &pane_key(address), r.pid, &r.start, table)
}

pub fn has_live_core(dir: &Path, core: &Core) -> bool {
    let table = SystemProcesses::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    load(dir)
        .records
        .iter()
        .filter(|r| {
            namespace_matches(r.pidns.as_deref()) && table.start(r.pid).as_deref() == Some(&r.start)
        })
        .any(|r| {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            !remaining.is_zero()
                && core
                    .with_timeout(remaining)
                    .process(&r.terminal)
                    .is_some_and(|info| {
                        if r.harness == "claude" {
                            proves_process(r, &r.terminal, &info, &table)
                        } else {
                            crate::codex::identity::core_owns_record(
                                &info,
                                &r.terminal,
                                r.pid,
                                &r.start,
                                &table,
                            )
                        }
                    })
        })
}

/// Same guard as the kit's process_alive: a supplied foreign namespace is unknown.
pub fn namespace_matches(recorded: Option<&str>) -> bool {
    recorded.is_none_or(|ns| herdr_agent_kit::agent_procs::own_pidns().as_deref() == Some(ns))
}

#[cfg(test)]
mod tests;
