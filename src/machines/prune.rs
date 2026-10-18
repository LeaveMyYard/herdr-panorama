//! Dropping mirrored sessions a machine stopped naming (Python `machines.prune`).
//!
//! `seen.json` in the mirror keeps when each session was last named (POSIX seconds). A
//! session not named for longer than the done window loses its transcript and its directory.
//! Only a well-formed session id is ever a reason to delete anything, and only a real
//! directory directly inside its project directory is removed (never through a symlink).

use super::valid_session;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// The bookkeeping file in the mirror.
pub const SEEN: &str = "seen.json";

/// `seen.json`: valid ids with a number (a JSON bool counts as 1 or 0, as in Python).
fn load_seen(path: &Path) -> Map<String, Value> {
    let read = fs::read_to_string(path).ok();
    let mut seen = match read.and_then(|t| serde_json::from_str(&t).ok()) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    seen.retain(|k, v| valid_session(k) && matches!(v, Value::Number(_) | Value::Bool(_)));
    seen
}

fn secs(v: &Value) -> f64 {
    match v {
        Value::Bool(b) => f64::from(u8::from(*b)),
        v => v.as_f64().unwrap_or(0.0),
    }
}

/// Drop the mirrored sessions not named for longer than `keep_seconds`; `sessions` are named
/// now. A transcript mirrored before this bookkeeping starts its clock now. Returns the ids
/// dropped. Errors are ignored: a later run tries again.
pub fn prune(dest: &Path, sessions: &[String], keep_seconds: f64, now: f64) -> Vec<String> {
    let path = dest.join(SEEN);
    let mut seen = load_seen(&path);
    for sid in sessions.iter().filter(|s| valid_session(s)) {
        seen.insert(sid.clone(), now.into());
    }
    let mut gone = Vec::new();
    for proj in dirs(dest) {
        let Ok(base) = proj.canonicalize() else {
            continue;
        };
        for sid in transcripts(&proj) {
            if let Some(at) = seen.get(&sid) {
                if now - secs(at) <= keep_seconds {
                    continue;
                }
            } else {
                seen.insert(sid, now.into());
                continue;
            }
            let d = proj.join(&sid);
            let link = d.symlink_metadata().is_ok_and(|m| m.is_symlink());
            let outside = d.exists()
                && d.canonicalize().ok().as_deref().and_then(Path::parent) != Some(&base);
            if link || outside {
                continue;
            }
            let _ = fs::remove_file(proj.join(format!("{sid}.jsonl")));
            if d.is_dir() {
                let _ = fs::remove_dir_all(&d);
            }
            seen.remove(&sid);
            gone.push(sid);
        }
    }
    let _ = fs::write(&path, Value::Object(seen).to_string());
    gone
}

/// The real directories (not symlinks) directly in `dest`.
fn dirs(dest: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(dest) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect()
}

/// The session ids of the `<id>.jsonl` files in `proj` that are not symlinks.
fn transcripts(proj: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(proj) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let sid = name.strip_suffix(".jsonl")?;
            valid_session(sid).then(|| sid.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests;
