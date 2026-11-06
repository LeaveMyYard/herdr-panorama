//! Reads the session records out of the hook files, within fixed bounds (session-linking.md
//! §3 H). The newest files go first, so stale ones cannot use up the budget of live ones.
use super::Record;
use crate::host::context::valid_address;
use herdr_agent_kit::agent_session::SessionId;
use serde_json::Value;
use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The hook records that were read.
pub struct Hooks {
    pub records: Vec<Record>,
    /// Every hook file was read to its end. Otherwise `records` may lack a process's newest
    /// session, and whichever file was left out may belong to any process.
    pub complete: bool,
}

struct Limits {
    /// Directory entries looked at.
    entries: usize,
    /// A longer file is not read: whole subagent lines can follow a session record, so no
    /// part of a file stands for the rest. It costs no budget.
    file: u64,
    /// Bytes read per load.
    total: u64,
}

const LIMITS: Limits = Limits {
    entries: 16 * 1024,
    file: 16 * 1024 * 1024,
    total: 32 * 1024 * 1024,
};

struct HookFile {
    modified: Option<SystemTime>,
    path: PathBuf,
    session: String,
    size: u64,
}

pub fn load(dir: &Path) -> Hooks {
    scan(dir, &LIMITS)
}

fn scan(dir: &Path, limits: &Limits) -> Hooks {
    let (mut files, mut complete) = list(dir, limits.entries);
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut records = Vec::new();
    let mut remaining = limits.total;
    let mut bytes = Vec::new();
    for file in files {
        let limit = limits.file.min(remaining);
        if file.size > limit {
            complete = false;
            continue;
        }
        bytes.clear();
        complete &= read(&file.path, limit, &mut bytes);
        remaining = remaining.saturating_sub(bytes.len() as u64);
        records.extend(
            bytes
                .split(|b| *b == b'\n')
                .filter_map(|line| parse(&file.session, line)),
        );
    }
    Hooks { records, complete }
}

/// The hook files of `dir`, and whether all of them were seen.
fn list(dir: &Path, max_entries: usize) -> (Vec<HookFile>, bool) {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => return (files, e.kind() == ErrorKind::NotFound),
    };
    let mut complete = true;
    for (seen, entry) in entries.enumerate() {
        if seen == max_entries {
            return (files, false);
        }
        let Ok(entry) = entry else {
            complete = false;
            continue;
        };
        let path = entry.path();
        let Some(session) = session_of(&path) else {
            continue;
        };
        match entry.metadata() {
            Ok(meta) if meta.is_file() => files.push(HookFile {
                modified: meta.modified().ok(),
                session,
                size: meta.len(),
                path,
            }),
            Ok(_) => {}
            Err(e) => complete &= e.kind() == ErrorKind::NotFound,
        }
    }
    (files, complete)
}

fn session_of(path: &Path) -> Option<String> {
    let stem = path
        .extension()
        .is_some_and(|e| e == "jsonl")
        .then(|| path.file_stem()?.to_str())??;
    SessionId::parse(stem).map(|_| stem.to_owned())
}

/// Whether the whole file fit in `limit` (a pruned file has nothing left to read). What was
/// read stays in `bytes` either way: the file may have grown since it was listed.
fn read(path: &Path, limit: u64, bytes: &mut Vec<u8>) -> bool {
    match File::open(path) {
        Ok(file) => file.take(limit + 1).read_to_end(bytes).is_ok() && bytes.len() as u64 <= limit,
        Err(e) => e.kind() == ErrorKind::NotFound,
    }
}

/// A core SessionStart record. Most lines are subagent records: both writers leave ASCII
/// unescaped, so a line without the bare word is not worth a JSON parse.
fn parse(session: &str, line: &[u8]) -> Option<Record> {
    const EVENT: &[u8] = b"\"session\"";
    if !line.windows(EVENT.len()).any(|w| w == EVENT) {
        return None;
    }
    let v = serde_json::from_slice::<Value>(line).ok()?;
    if v["v"] != 1 || v["event"] != "session" || v["host"] != "herdr-core" {
        return None;
    }
    let terminal = v["terminal"].as_str()?;
    let pid = v["pid"].as_u64()?;
    let start = v["pstart"].as_str()?;
    let harness = v["harness"].as_str()?;
    if !valid_address(terminal)
        || terminal.contains('/')
        || pid <= 1
        || pid > u64::from(u32::MAX)
        || start.is_empty()
        || !matches!(harness, "claude" | "codex")
    {
        return None;
    }
    let pidns = match v.get("pidns") {
        Some(Value::String(ns)) => Some(ns.clone()),
        Some(Value::Null) | None => None,
        _ => return None,
    };
    Some(Record {
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
    })
}

#[cfg(test)]
mod tests;
