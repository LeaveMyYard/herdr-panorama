//! What the manager keeps across core restarts (Python `Manager._load`/`_save_closed`), in
//! `panels-<session>.json` in the state dir: the tabs whose panel the operator closed, and our
//! running panels (pane id → tab and herdr's terminal id) for after a herdr restart. Read as
//! leniently as Python reads it; written whole, through a temporary file.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A panel of ours as last seen running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    pub tab: String,
    pub terminal: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    pub closed: BTreeSet<String>,
    pub panes: BTreeMap<String, Recorded>,
}

/// The saved state at `path` (nothing when it is missing or unreadable).
pub fn load(path: Option<&Path>) -> Saved {
    let data: Value = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let closed = data["closed"].as_array().map_or_else(BTreeSet::new, |a| {
        a.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect()
    });
    let panes = data["panes"].as_object().map_or_else(BTreeMap::new, |o| {
        o.iter()
            .filter_map(|(p, r)| {
                let r = r.as_object()?;
                Some((p.clone(), Recorded { tab: text(r, "tab"), terminal: text(r, "terminal") }))
            })
            .collect()
    });
    Saved { closed, panes }
}

fn text(r: &Map<String, Value>, k: &str) -> String {
    r.get(k).and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Saves `saved` at `path`; why not, if it failed.
pub fn save(path: &Path, saved: &Saved) -> Result<(), String> {
    let panes: Map<String, Value> =
        saved.panes.iter().map(|(p, r)| (p.clone(), json!({"tab": r.tab, "terminal": r.terminal}))).collect();
    let doc = json!({"closed": saved.closed.iter().collect::<Vec<_>>(), "panes": panes});
    let tmp: PathBuf = path.with_extension("tmp");
    let write = || -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&tmp, doc.to_string())?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| format!("could not save the panels' state: {e}"))
}
