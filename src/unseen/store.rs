//! The unseen state file (Python `Tracker._read`, `_write`, `_locked`): one JSON file shared by
//! every panorama process of the machine (popup, home tab, panels), kept across restarts,
//! changed under a short lock beside it and replaced atomically.

use herdr_agent_kit::plugin_env::LockFile;
use serde_json::{Map, Value};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a refresh waits for another process's lock before it keeps its last marks.
pub const LOCK_WAIT: Duration = Duration::from_millis(200);
const POLL: Duration = Duration::from_millis(10);

/// The state, or `None`: no file, or one that is not this format (a broken file starts over).
pub fn read(path: &Path) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) if m.get("agents").is_some_and(Value::is_object) => Some(m),
        _ => None,
    }
}

/// Write it whole: a temp file beside it (this process's), renamed into place.
pub fn write(path: &Path, data: &Map<String, Value>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, Value::Object(data.clone()).to_string())?;
    std::fs::rename(&tmp, path)
}

/// `f` under the file's lock; `Ok(None)` when another process holds it past [`LOCK_WAIT`].
pub fn locked<T>(path: &Path, f: impl FnOnce() -> io::Result<T>) -> io::Result<Option<T>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let held = LockFile::new(PathBuf::from(lock))
        .mode(0o644)
        .exclusive_within(LOCK_WAIT, POLL, || true)?;
    match held {
        Some(_held) => f().map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
