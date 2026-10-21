//! Appending a declaration to a session's events file (Python `events.emit`, `_compact`).
//! Writers hold an exclusive lock on `<file>.lock` (mode 0644) for the append and any
//! compaction, so a compaction never drops another writer's line. The append is one write
//! with `O_APPEND`, so a reader never sees half a line.

use super::{COMPACT_AT, stamp};
use crate::collect::events::{Channel, Declared};
use crate::snapshot::pyjson::{J, dumps_line};
use herdr_agent_kit::plugin_env::LockFile;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The channel's format version.
const VERSION: i64 = 1;

/// `<file>.lock` beside the events file.
fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Append `record` (its fields after `v` and `ts`) to the events file at `path`, stamped at
/// `now`; compact the file when it grew past [`COMPACT_AT`].
pub fn emit(path: &Path, record: Vec<(String, J)>, now: f64) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let _held = LockFile::new(lock_path(path)).mode(0o644).exclusive()?;
    // Python's `{"v": …, "ts": …, **record}`: a record's own `v` or `ts` replaces these in place
    let mut fields = vec![
        ("v".to_owned(), J::Int(VERSION)),
        ("ts".to_owned(), J::Str(stamp(now))),
    ];
    for (k, v) in record {
        match fields.iter_mut().find(|(key, _)| *key == k) {
            Some(slot) => slot.1 = v,
            None => fields.push((k, v)),
        }
    }
    let line = dumps_line(&J::Obj(fields)) + "\n";
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o644)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    if fs::metadata(path)?.len() > COMPACT_AT {
        compact(path)?;
    }
    Ok(())
}

fn maps(items: &[serde_json::Map<String, serde_json::Value>]) -> J {
    J::Arr(
        items
            .iter()
            .map(|m| J::from(&serde_json::Value::Object(m.clone())))
            .collect(),
    )
}

/// One full declaration of a thread.
fn declaration(d: &Declared) -> J {
    let mut rec = vec![
        ("v".to_owned(), J::Int(VERSION)),
        ("thread".to_owned(), J::str(&d.thread)),
        ("phases".to_owned(), maps(&d.phases)),
        ("agents".to_owned(), maps(&d.agents)),
    ];
    if let Some(ts) = d.ts.filter(|t| *t != 0.0) {
        rec.push(("ts".to_owned(), J::Str(stamp(ts))));
    }
    let text = [
        ("name", &d.name),
        ("flavour", &d.flavour),
        ("status", &d.status),
        ("worktree", &d.worktree),
    ];
    rec.extend(
        text.into_iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| (k.to_owned(), J::str(v)))),
    );
    if let Some(pct) = d.pct {
        rec.push(("pct".to_owned(), J::Int(pct)));
    }
    J::Obj(rec)
}

/// Rewrite the file as one full declaration per thread, through a temp file renamed into
/// place (a new inode: readers read it again from the start). The caller holds the lock.
pub fn compact(path: &Path) -> io::Result<()> {
    let mut ch = Channel::new(path.to_path_buf());
    ch.update();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let text: String = ch
        .threads
        .iter()
        .map(|d| dumps_line(&declaration(d)) + "\n")
        .collect();
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests;
