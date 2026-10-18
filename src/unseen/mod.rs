//! What the operator has not seen yet (Python `unseen.py`, design turn 6): a new failure, a new
//! question or a finished agent marks that agent unseen until the operator clicks it, presses
//! Enter on it, docks next to its orchestrator, or moves herdr's focus onto the orchestrator's
//! pane. The views pulse unseen cards.
//!
//! The state lives in one JSON file under panorama's state directory ([`store`]), shared by
//! every panorama process of the machine and kept across restarts:
//!
//! ```text
//! {"v": 1, "saved": <time>, "agents": {"<agent id>": {"s": <status>, "q": [<question ids>],
//!                                                      "ev": "fail"|"wait"|"done"|null, "t": <last seen>}}}
//! ```
//!
//! Each refresh compares the swarm with the file under a short lock: a status change or a new
//! question id since the file's record is an event ([`rules`]), so two processes never report
//! one event twice, and a mark cleared in one process is cleared in all. The first run (no
//! file) only records what is there.

pub mod rules;
pub mod store;
mod update;

pub use rules::Event;

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// Agents not seen for this long are forgotten.
pub const KEEP_SECONDS: f64 = 7.0 * 86_400.0;

/// The marks: agent id → its most urgent unseen event.
pub type Marks = BTreeMap<String, Event>;

/// `highlight_unseen` in panorama's config: a bool, else on.
pub fn enabled(cfg: &Map<String, Value>) -> bool {
    cfg.get("highlight_unseen")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// The state file of this herdr server: `<state dir>/unseen-<session key>.json`.
pub fn default_path(env: &herdr_agent_kit::plugin_env::PluginEnv) -> PathBuf {
    use herdr_agent_kit::internal::plugin_paths::panorama::{session_key, state_dir};
    state_dir(env).join(format!("unseen-{}.json", session_key(env)))
}

#[derive(Debug, Default)]
struct Inner {
    marks: Marks,
    /// Seen here, not written yet (the lock was busy).
    pending: BTreeSet<String>,
    /// Orchestrator panes herdr had focused at the last update (`None`: no update yet).
    prev_focused: Option<BTreeSet<String>>,
    /// Agent id → its state file key (machine-prefixed if remote).
    keymap: BTreeMap<String, String>,
}

/// One process's view of the unseen state: the collect worker updates it, the UI marks seen.
pub struct Tracker {
    path: PathBuf,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
    inner: Mutex<Inner>,
}

impl Tracker {
    /// A tracker of the state file at `path`; `enabled` says whether marks are on now.
    pub fn new(path: PathBuf, enabled: Box<dyn Fn() -> bool + Send + Sync>) -> Self {
        Self {
            path,
            enabled,
            inner: Mutex::default(),
        }
    }

    /// The state file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The marks now.
    pub fn marks(&self) -> Marks {
        self.inner().marks.clone()
    }

    /// The operator looked at these agents: their marks clear here at once. Returns the ids
    /// whose mark cleared, for [`Tracker::save_seen`] (file IO: off the UI thread).
    pub fn seen<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
        let mut inner = self.inner();
        let cleared: BTreeSet<String> = ids
            .into_iter()
            .filter(|i| inner.marks.contains_key(*i))
            .map(str::to_owned)
            .collect();
        for i in &cleared {
            inner.marks.remove(i);
        }
        inner.pending.extend(cleared.iter().cloned());
        cleared
    }

    /// Write cleared marks to the file. A busy lock leaves them pending: the next update
    /// clears them. Errors are ignored the same way (Python logs them).
    pub fn save_seen(&self, ids: &BTreeSet<String>) {
        if ids.is_empty() {
            return;
        }
        let keys: BTreeSet<String> = {
            let inner = self.inner();
            ids.iter()
                .map(|i| inner.keymap.get(i).cloned().unwrap_or_else(|| i.clone()))
                .collect()
        };
        let _ = store::locked(&self.path, || {
            if let Some(mut data) = store::read(&self.path) {
                if let Some(Value::Object(agents)) = data.get_mut("agents") {
                    for k in &keys {
                        if let Some(Value::Object(rec)) = agents.get_mut(k) {
                            rec.insert("ev".into(), Value::Null);
                        }
                    }
                }
                store::write(&self.path, &data)?;
            }
            let mut inner = self.inner();
            for i in ids {
                inner.pending.remove(i);
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests;
