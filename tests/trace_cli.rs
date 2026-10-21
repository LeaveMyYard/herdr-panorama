//! `PANORAMA_TRACE` in a real process: `panorama snapshot` (the blocking client) traces its
//! herdr call; and the kit's client types are named only in `src/trace.rs`, so every client
//! panorama makes comes from `trace::client` or `blocking_client`.

mod collect_support;

use collect_support::Scenario;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn the_snapshot_process_traces_its_herdr_call() {
    let sc = Scenario::new("demo");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot": sc.snapshot()})),
    );
    let trace = sc.root().join("trace.jsonl");
    let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .env_clear()
        .args(["snapshot", "--json"])
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", sc.root())
        .env("HERDR_SOCKET_PATH", fake.socket())
        .env("HERDR_BIN_PATH", "/nonexistent/herdr")
        .env("HERDR_PLUGIN_CONFIG_DIR", sc.root().join("config"))
        .env("CLAUDE_CONFIG_DIR", sc.root())
        .env("PANORAMA_EVENTS_DIR", sc.root().join("events"))
        .env("PANORAMA_HOOKS_DIR", sc.root().join("hooks"))
        .env("HERDR_INBOX_DB", sc.root().join("missing.db"))
        .env("PANORAMA_TRACE", &trace)
        .output()
        .unwrap();
    assert!(out.status.success());
    let ops: Vec<String> = std::fs::read_to_string(&trace)
        .expect("a trace file")
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["op"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert!(
        ops.iter().any(|o| o == "socket session.snapshot"),
        "{ops:?}"
    );
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "tests" && name != "herdr_panorama" {
                sources(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(p);
        }
    }
}

#[test]
fn no_herdr_client_is_named_outside_trace() {
    // the kit's client types (`Herdr`, `blocking::Herdr`) appear only in src/trace.rs: anywhere
    // else, even under an alias in a `use`, a client could be made untraced
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    sources(&root, &mut files);
    assert!(files.len() > 50, "the scan reads panorama's sources");
    let word = regex::Regex::new(r"\bHerdr\b").unwrap();
    let mut found = vec![];
    for f in files.iter().filter(|f| **f != root.join("trace.rs")) {
        for (n, line) in std::fs::read_to_string(f).unwrap().lines().enumerate() {
            if word.is_match(line) {
                found.push(format!(
                    "{}:{}: {}",
                    f.strip_prefix(&root).unwrap().display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "use crate::trace::{{HerdrClient, BlockingHerdrClient}} and trace::client():\n{}",
        found.join("\n")
    );
}
