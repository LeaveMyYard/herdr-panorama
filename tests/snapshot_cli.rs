//! `panorama snapshot` as a process: its herdr is a fake socket, its transcripts the captured
//! demo scenario, its environment cleared (never the session's herdr).

mod collect_support;

use collect_support::Scenario;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn snapshot(args: &[&str], socket: &Path, sc: &Scenario) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .env_clear()
        .arg("snapshot")
        .args(args)
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", sc.root())
        .env("HERDR_SOCKET_PATH", socket)
        .env("HERDR_BIN_PATH", "/nonexistent/herdr")
        .env("HERDR_PLUGIN_CONFIG_DIR", sc.root().join("config"))
        .env("CLAUDE_CONFIG_DIR", sc.root())
        .env("PANORAMA_EVENTS_DIR", sc.root().join("events"))
        .env("PANORAMA_HOOKS_DIR", sc.root().join("hooks"))
        .env("HERDR_INBOX_DB", sc.root().join("missing.db"))
        .env("PANORAMA_DONE_WINDOW", "100000000")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
    )
}

#[test]
fn the_json_snapshot_reads_herdr_and_the_transcripts() {
    let sc = Scenario::new("demo");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot": sc.snapshot()})),
    );
    let (code, out) = snapshot(&["--json"], fake.socket(), &sc);
    assert_eq!(code, 0);
    let got: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (got["herdr_ok"].clone(), got["inbox_ok"].clone()),
        (json!(true), json!(false))
    );
    let labels: Vec<&str> = got["orchestrators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["tab_label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "Billing API",
            "Search refactor",
            "Docs site",
            "Release notes"
        ]
    );
    assert!(out.starts_with("{\n  \"generated_at\": ") && out.ends_with("}\n"));
    let calls: Vec<String> = fake.calls().into_iter().map(|(m, _)| m).collect();
    assert!(
        calls
            .iter()
            .all(|m| m == "session.snapshot" || m == "pane.process_info"),
        "{calls:?}"
    );
}

#[test]
fn herdr_down_still_prints_a_model_and_counts_print_their_totals() {
    let sc = Scenario::new("demo");
    let (code, out) = snapshot(&["--json"], &sc.root().join("no.sock"), &sc);
    assert_eq!(code, 0);
    let got: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(got["herdr_ok"], json!(false));
    assert_eq!(got["orchestrators"], json!([]));
    let (_, counts) = snapshot(&["--counts"], &sc.root().join("no.sock"), &sc);
    let c: Value = serde_json::from_str(&counts).unwrap();
    let keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
    for k in [
        "orchestrators",
        "herdr_ok",
        "by_source",
        "agent_status",
        "hidden_done_threads",
        "seconds",
    ] {
        assert!(keys.contains(&k), "{k} in {keys:?}");
    }
    let (code, _) = snapshot(&["--yaml"], &sc.root().join("no.sock"), &sc);
    assert_eq!(code, 2);
}

#[test]
fn the_text_listing_names_each_orchestrator() {
    let sc = Scenario::new("demo");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot": sc.snapshot()})),
    );
    let (_, out) = snapshot(&[], fake.socket(), &sc);
    let heads: Vec<&str> = out.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(heads.len(), 4, "{out}");
    assert!(
        heads[0].starts_with("Billing API  [working] "),
        "{}",
        heads[0]
    );
    // the scenario is days old by the real clock: its threads read as finished or failed
    assert!(
        out.lines()
            .any(|l| l.starts_with("  ✓ #42  (ref, done, 4 agents)")),
        "{out}"
    );
    assert!(
        out.lines()
            .any(|l| l.starts_with("      ") && l.contains(" fixer ")),
        "{out}"
    );
}
