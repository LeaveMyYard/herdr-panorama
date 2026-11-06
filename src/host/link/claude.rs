//! Evidence P: Claude Code's own per-process session record, `<config>/sessions/<pid>.json`.
//! Claude writes `{pid, sessionId, procStart, …}` at startup, rewrites `sessionId` on every
//! session change and removes the file on exit (session-linking.md §3 P, V1).
use super::Signal;
use herdr_agent_kit::agent_session::SessionId;
use rustix::fs::{Mode, OFlags};
use serde_json::Value;
use std::{
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 16 * 1024;
const MAX_RECORDS: usize = 4096;

/// The session the record of process `pid` (start token `start`) names, else `Absent`.
pub fn record(config: &Path, pid: u32, start: &str, uid: u32) -> Signal {
    let path = config.join("sessions").join(format!("{pid}.json"));
    let Ok(fd) = rustix::fs::open(
        &path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return Signal::Absent;
    };
    let file = File::from(fd);
    let Ok(meta) = file.metadata() else {
        return Signal::Absent;
    };
    if !meta.file_type().is_file()
        || meta.uid() != uid
        || meta.nlink() != 1
        || meta.len() > MAX_BYTES
    {
        return Signal::Absent;
    }
    let mut bytes = Vec::new();
    if file.take(MAX_BYTES + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_BYTES {
        return Signal::Absent;
    }
    parse(&bytes, pid, start)
}

/// Claude config dirs in `home`: `.claude` and any `.claude-*`. Each process's record sits in
/// its own dir, so the record says which dir a process uses, with no need for its environment.
pub fn config_dirs(home: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    let named = |name: &str| name == ".claude" || name.starts_with(".claude-");
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(named))
        .map(|e| e.path())
        .filter(|p| p.join("sessions").is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// True if a live process has `session`: its record's pid is running with the recorded
/// start token. A record left by a dead process proves nothing.
pub fn held(dirs: &[PathBuf], session: &str, start: &dyn Fn(u32) -> Option<String>) -> bool {
    let records = dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir.join("sessions")).ok())
        .flatten()
        .flatten();
    records.take(MAX_RECORDS).any(|entry| {
        let Ok(bytes) = std::fs::read(entry.path()) else {
            return false;
        };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        let pid = v
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|p| u32::try_from(p).ok());
        v.get("sessionId").and_then(Value::as_str) == Some(session)
            && pid.and_then(start).as_deref() == v.get("procStart").and_then(Value::as_str)
            && pid.is_some()
    })
}

/// `pid` and `procStart` must be this exact process (a record left by an earlier process
/// with a reused PID carries another start token), and `kind` the plain interactive TUI.
pub fn parse(bytes: &[u8], pid: u32, start: &str) -> Signal {
    let Ok(Value::Object(v)) = serde_json::from_slice(bytes) else {
        return Signal::Absent;
    };
    let same = v.get("pid").and_then(Value::as_u64) == Some(u64::from(pid))
        && v.get("procStart").and_then(Value::as_str) == Some(start)
        && v.get("kind").and_then(Value::as_str) == Some("interactive");
    match v.get("sessionId").and_then(Value::as_str) {
        Some(id) if same && SessionId::parse(id).is_some() => Signal::Names(id.to_owned()),
        _ => Signal::Absent,
    }
}
