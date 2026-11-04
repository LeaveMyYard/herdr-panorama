//! Evidence P: Claude Code's own per-process session record, `<config>/sessions/<pid>.json`.
//! Claude writes `{pid, sessionId, procStart, …}` at startup, rewrites `sessionId` on every
//! session change and removes the file on exit (session-linking.md §3 P, V1).
use super::Signal;
use herdr_agent_kit::agent_session::SessionId;
use rustix::fs::{FileType, Mode, OFlags};
use serde_json::Value;
use std::{fs::File, io::Read, os::unix::fs::MetadataExt, path::Path};

const MAX_BYTES: u64 = 16 * 1024;

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
    if FileType::from_raw_mode(meta.mode()) != FileType::RegularFile
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
