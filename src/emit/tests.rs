use super::*;
use crate::collect::events::Channel;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn env(dir: &std::path::Path, session: Option<&str>) -> PluginEnv {
    let mut vars = vec![("PANORAMA_EVENTS_DIR".to_owned(), dir.display().to_string())];
    if let Some(s) = session {
        vars.push(("CLAUDE_CODE_SESSION_ID".to_owned(), s.to_owned()));
    }
    PluginEnv::from_vars(vars, None, 1000)
}

#[test]
fn a_declaration_is_written_where_the_collector_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let argv = args(&[
        "--thread",
        "pr-7",
        "--name",
        "PR #7",
        "--flavour",
        "pr-review",
        "--status",
        "active",
        "--pct",
        "60",
        "--phase",
        "author=done",
        "--phase",
        "review r1=now",
        "--agent",
        "Review PR*=reviewer@1",
        "--json",
        r#"{"worktree": "/w", "name": "PR #7 (renamed)"}"#,
    ]);
    let path = emit(&argv, &env(dir.path(), Some("s1")), 1_791_960_000.0).unwrap();
    assert_eq!(path, dir.path().join("s1.jsonl"));
    let line = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        line,
        "{\"v\": 1, \"ts\": \"2026-10-14T06:40:00Z\", \"thread\": \"pr-7\", \"name\": \"PR #7 (renamed)\", \"flavour\": \"pr-review\", \"status\": \"active\", \"pct\": 60, \"phases\": [{\"name\": \"author\", \"state\": \"done\"}, {\"name\": \"review r1\", \"state\": \"now\"}], \"agents\": [{\"match\": \"Review PR*\", \"role\": \"reviewer\", \"phase\": 1}], \"worktree\": \"/w\"}\n"
    );
    let mut ch = Channel::new(path);
    ch.update();
    let d = &ch.threads[0];
    assert_eq!(
        (d.name.as_deref(), d.pct, d.phases.len(), d.agents.len()),
        (Some("PR #7 (renamed)"), Some(60), 2, 1)
    );
}

#[test]
fn the_session_comes_from_the_flag_else_claude_code() {
    let dir = tempfile::tempdir().unwrap();
    let p = emit(
        &args(&["--thread", "t", "--session", "s9"]),
        &env(dir.path(), Some("s1")),
        0.0,
    )
    .unwrap();
    assert_eq!(p, dir.path().join("s9.jsonl"));
    let p = emit(
        &args(&["--thread", "t", "--session", ""]),
        &env(dir.path(), Some("s1")),
        0.0,
    )
    .unwrap();
    assert_eq!(p, dir.path().join("s1.jsonl")); // Python's `or`: an empty flag is no flag
    let err = emit(&args(&["--thread", "t"]), &env(dir.path(), None), 0.0).unwrap_err();
    assert_eq!(err.0, 2);
    assert!(err.1.contains("no session"));
    assert_eq!(
        emit(
            &args(&["--thread", "t", "--json", "[1]"]),
            &env(dir.path(), Some("s")),
            0.0
        )
        .unwrap_err()
        .0,
        2
    );
    assert_eq!(
        emit(&args(&["--name", "x"]), &env(dir.path(), Some("s")), 0.0)
            .unwrap_err()
            .0,
        2
    );
}
