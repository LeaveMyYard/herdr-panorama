//! The unseen state file (Python `Tracker._read`, `_write`, `_locked`): one JSON file shared by
//! every panorama process of the machine (popup, home tab, panels), kept across restarts,
//! changed under a short lock beside it and replaced atomically.

use crate::snapshot::pyjson::{J, Style, dumps_styled};
use herdr_agent_kit::plugin_env::LockFile;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a refresh waits for another process's lock before it keeps its last marks.
pub const LOCK_WAIT: Duration = Duration::from_millis(200);
const POLL: Duration = Duration::from_millis(10);

/// How Python writes the file: `json.dumps(data, separators=(",", ":"))`.
pub const STYLE: Style = Style {
    item: ",",
    key: ":",
    ascii: true,
};

/// The state, keys in file order, or `None`: no file, or one that is not this format (a broken
/// file starts over).
pub fn read(path: &Path) -> Option<J> {
    let j = J::parse(&std::fs::read_to_string(path).ok()?)?;
    (j.is_obj() && j.get("agents").is_some_and(J::is_obj)).then_some(j)
}

/// Write it whole, byte for byte as Python writes it: a temp file beside it (this process's),
/// renamed into place.
pub fn write(path: &Path, data: &J) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, dumps_styled(data, STYLE))?;
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
