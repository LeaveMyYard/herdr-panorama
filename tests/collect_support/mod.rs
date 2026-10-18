//! The captured `tests/scenario.py` scenarios (`tests/fixtures/inputs/scenario-*.json`),
//! materialised as `tools/capture_goldens.py` does: the files with their mtimes under a root,
//! the events dir, the herdr snapshot and the inbox rows.

#![allow(dead_code)]

use herdr_agent_kit::internal::inbox_contract::sqlite::OpenQuestion;
use panorama::collect::{Collector, Options};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

pub const INPUTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/inputs");
pub const MODELS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/model");
/// `config.DEFAULT_DONE_WINDOW`, `DEFAULT_STALE_AFTER`.
pub const WINDOW: f64 = 3600.0;
pub const STALE: f64 = 12.0 * 3600.0;
/// The demo's sessions (`scenario.S1`) and its shell-free panes.
pub const S1: &str = "11111111-1111-4111-8111-111111111111";

pub fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A scenario materialised in its own temp dir.
pub struct Scenario {
    pub input: Value,
    pub dir: tempfile::TempDir,
}

impl Scenario {
    /// `scenario-<name>.json`, its files written with their mtimes.
    pub fn new(name: &str) -> Self {
        Self::load(&Path::new(INPUTS).join(format!("scenario-{name}.json")))
    }

    /// A scenario file in the same shape, materialised.
    pub fn load(path: &Path) -> Self {
        let input = read_json(path);
        let dir = tempfile::Builder::new()
            .prefix("scenario")
            .tempdir()
            .unwrap();
        for (rel, f) in input["files"].as_object().expect("files") {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).unwrap();
            std::fs::write(&path, f["text"].as_str().expect("text")).unwrap();
            let mtime = UNIX_EPOCH + Duration::from_secs_f64(f["mtime"].as_f64().expect("mtime"));
            std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|fh| fh.set_modified(mtime))
                .unwrap();
        }
        std::fs::create_dir_all(dir.path().join(input["events"].as_str().unwrap())).unwrap();
        Self { input, dir }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn now(&self) -> f64 {
        self.input["now"].as_f64().unwrap()
    }

    pub fn projects(&self) -> PathBuf {
        self.root().join(self.input["projects"].as_str().unwrap())
    }

    pub fn snapshot(&self) -> &Value {
        &self.input["snapshot"]
    }

    /// A collector over it (no process checks: the capture's herdr names no process).
    pub fn collector(&self) -> Collector {
        Collector::new(Options {
            projects: self.projects(),
            events_dir: self.root().join(self.input["events"].as_str().unwrap()),
            hooks_dir: self.root().join("hooks"),
            procs: true,
            pane_pid: Box::new(|_| None),
        })
    }

    /// The inbox rows `inbox.open_questions` returns: `status = 'open'`, by id.
    pub fn questions(&self) -> Vec<OpenQuestion> {
        let mut rows: Vec<&Value> = self.input["inbox"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["status"] == "open")
            .collect();
        rows.sort_by_key(|r| r["id"].as_i64());
        rows.into_iter()
            .map(|r| OpenQuestion {
                id: r["id"].as_i64().unwrap(),
                session_id: r["session"].as_str().map(str::to_owned),
                pane_id: r["pane"].as_str().map(str::to_owned),
                asked_by: r["asked_by"].as_str().map(str::to_owned),
                question: r["question"].as_str().unwrap_or("").to_owned(),
                created_at: None,
            })
            .collect()
    }
}
