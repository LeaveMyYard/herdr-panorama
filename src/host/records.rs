//! Read only recorded addresses; cwd, labels and recency never select a session.
use super::{Core, context::pane_key, core::owns, link::Signal};
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    agent_session::SessionId,
};
use serde_json::Value;
use std::{collections::HashSet, io::Read, path::Path};

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

pub fn load(dir: &Path) -> Vec<Record> {
    let mut records = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return records;
    };
    let mut remaining = 8 * 1024 * 1024_u64;
    for entry in entries.take(512).flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl")
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            continue;
        }
        let Some(session) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if SessionId::parse(session).is_none() {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let size = file.metadata().map_or(u64::MAX, |m| m.len());
        if size > remaining || size > 256 * 1024 {
            continue;
        }
        remaining -= size;
        let mut bytes = Vec::new();
        if file.take(size + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 != size {
            continue;
        }
        for line in bytes.split(|b| *b == b'\n') {
            let Ok(v) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            if v["v"] != 1 || v["event"] != "session" || v["host"] != "herdr-core" {
                continue;
            }
            let (Some(terminal), Some(pid), Some(start), Some(harness)) = (
                v["terminal"].as_str(),
                v["pid"].as_u64(),
                v["pstart"].as_str(),
                v["harness"].as_str(),
            ) else {
                continue;
            };
            if !super::context::valid_address(terminal)
                || terminal.contains('/')
                || pid <= 1
                || pid > u64::from(u32::MAX)
                || start.is_empty()
                || !matches!(harness, "claude" | "codex")
            {
                continue;
            }
            let pidns = match v.get("pidns") {
                Some(Value::String(ns)) => Some(ns.clone()),
                Some(Value::Null) | None => None,
                _ => continue,
            };
            records.push(Record {
                session: session.into(),
                terminal: terminal.into(),
                harness: harness.into(),
                pid: pid as u32,
                start: start.into(),
                pidns,
                ts: v["ts"]
                    .as_f64()
                    .filter(|t| t.is_finite())
                    .unwrap_or(f64::NAN),
            });
        }
    }
    records
}

/// Evidence H (session-linking.md §3): the sessions named by root SessionStart records of the
/// terminal's current Claude process. Several are ambiguous; the newest is kept only so that
/// Claude's own per-process record can corroborate it.
pub fn claude_session(
    records: &[Record],
    address: &str,
    info: &Value,
    table: &dyn ProcessTable,
) -> Signal {
    let mut ids = HashSet::new();
    let mut newest: Option<&Record> = None;
    let (mut ordered, mut tie) = (true, false);
    for r in records
        .iter()
        .filter(|r| r.terminal == address && r.harness == "claude")
    {
        let argv = table.cmdline(r.pid);
        let name = table.stat(r.pid).map_or(String::new(), |p| p.comm);
        let is_claude = herdr_agent_kit::agent_procs::claude::first_claude([(
            r.pid,
            name.as_str(),
            argv.as_slice(),
        )])
        .is_some();
        if is_claude
            && namespace_matches(r.pidns.as_deref())
            && owns(info, &pane_key(address), r.pid, &r.start, table)
        {
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
        (1, Some(r)) => Signal::Names(r.session.clone()),
        (_, Some(r)) if ordered && !tie => Signal::Newest(r.session.clone()),
        _ => Signal::Ambiguous,
    }
}

pub fn has_live_core(dir: &Path, core: &Core) -> bool {
    let table = SystemProcesses::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    load(dir)
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
                            matches!(
                                claude_session(std::slice::from_ref(r), &r.terminal, &info, &table),
                                Signal::Names(_)
                            )
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
