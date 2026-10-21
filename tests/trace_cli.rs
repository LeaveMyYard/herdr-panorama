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

/// The lines of `text` that could make a herdr client outside `src/trace.rs`: the kit's types
/// named at all (even in a `use`, so no alias dodges it), or a constructor called through
/// trace's names. Methods on those names (`BlockingHerdrClient::machine_snapshot`) are fine.
fn offending(text: &str) -> Vec<(usize, String)> {
    let kit = regex::Regex::new(r"\bHerdr\b").unwrap();
    let made =
        regex::Regex::new(r"\b(HerdrClient|BlockingHerdrClient)\s*::\s*(new|from_env|resolve)\b")
            .unwrap();
    text.lines()
        .enumerate()
        .filter(|(_, l)| kit.is_match(l) || made.is_match(l))
        .map(|(n, l)| (n + 1, l.trim().to_owned()))
        .collect()
}

#[test]
fn the_scan_flags_what_it_is_for() {
    // planted lines: each way to make a client outside trace.rs, and the ones that are fine
    let bad = [
        "use herdr_agent_kit::herdr_api::Herdr as H;",
        "let h = herdr_agent_kit::herdr_api::blocking::Herdr::resolve(var);",
        "let h = crate::trace::HerdrClient::from_env();",
        "let h = BlockingHerdrClient::new(server);",
        "let h = HerdrClient :: resolve(var);",
    ];
    for line in bad {
        assert_eq!(offending(line).len(), 1, "not flagged: {line}");
    }
    let fine = [
        "fn f(h: &HerdrClient) {}",
        "BlockingHerdrClient::machine_snapshot(self, id)",
        "let h = crate::trace::client();",
        "use herdr_agent_kit::herdr_api::{HerdrEvent, HerdrError};",
    ];
    for line in fine {
        assert!(offending(line).is_empty(), "flagged: {line}");
    }
}

#[test]
fn no_herdr_client_is_made_outside_trace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    sources(&root, &mut files);
    assert!(files.len() > 50, "the scan reads panorama's sources");
    let mut found = vec![];
    for f in files.iter().filter(|f| **f != root.join("trace.rs")) {
        for (n, line) in offending(&std::fs::read_to_string(f).unwrap()) {
            found.push(format!(
                "{}:{n}: {line}",
                f.strip_prefix(&root).unwrap().display()
            ));
        }
    }
    assert!(
        found.is_empty(),
        "make herdr clients with trace::client() / blocking_client():\n{}",
        found.join("\n")
    );
}
