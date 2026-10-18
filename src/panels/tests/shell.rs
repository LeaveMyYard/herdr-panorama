//! Reading a pane's foreground (Python `tests/test_panels.py`): `ps` elapsed times, the
//! `ps`/`pgrep` looks, and the plain-shell decision on herdr 0.9.3's answers.

use crate::panels::Shell;
use crate::panels::shell::{
    etime_seconds, plain_shell, runs_panorama, shell_state, shell_state_by,
};
use herdr_agent_kit::herdr_api::ProcessInfo;
use serde_json::{Value, json};

#[test]
fn ps_elapsed_time_parses() {
    for (text, secs) in [
        ("00:07", Some(7.0)),
        ("01:02:03", Some(3723.0)),
        ("2-00:00:01", Some(172801.0)),
        (" 59:59\n", Some(3599.0)),
        ("", None),
        ("x:1", None),
        ("1:2:3:4", None),
        ("7", None),
    ] {
        assert_eq!(etime_seconds(text), secs, "{text:?}");
    }
}

#[test]
fn shell_state_reads_ps_and_pgrep() {
    let by = |ps: (i32, &'static str), pgrep: (i32, &'static str)| {
        shell_state_by(42, move |prog, args| {
            assert!(args.contains(&"42"));
            let (c, o) = if prog == "ps" { ps } else { pgrep };
            Some((c, o.to_string()))
        })
    };
    assert_eq!(by((0, " 00:05\n"), (1, "")), (Some(5.0), Some(false)));
    assert_eq!(by((0, " 00:05\n"), (0, "43\n")), (Some(5.0), Some(true)));
    assert_eq!(by((1, ""), (2, "")), (None, None));
    assert_eq!(shell_state_by(42, |_, _| None), (None, None));
}

#[test]
fn shell_state_looks_at_a_real_process() {
    let (age, _) = shell_state(i64::from(std::process::id()));
    assert!(age.is_some_and(|a| a < 3600.0));
}

fn info(procs: Value) -> ProcessInfo {
    serde_json::from_value(
        json!({"pane_id": "w1:p2", "shell_pid": 73505, "foreground_processes": procs}),
    )
    .unwrap()
}

fn one(argv: &[&str], pid: i64) -> Value {
    json!([{"argv": argv, "name": "x", "pid": pid}])
}

#[test]
fn a_plain_shell_is_only_the_panes_own_shell() {
    // process-info as herdr 0.9.3 answered it on an isolated server: a live panel (its sh -c
    // execs panorama as the pane's shell pid) and the same pane restored as a login shell
    let mut live: ProcessInfo = serde_json::from_value(json!({"pane_id": "w1:p2", "shell_pid": 71225,
        "foreground_processes": [{"argv": ["/p/.venv/bin/python", "/p/.venv/bin/panorama", "panel"], "name": "panorama", "pid": 71225}]}))
    .unwrap();
    assert_eq!(plain_shell(&live), Shell::Busy, "live-panel");
    assert_eq!(runs_panorama(&live, "panel"), Some(true));
    assert_eq!(runs_panorama(&live, "tui"), Some(false));
    live.foreground_processes = Some(vec![]);
    assert_eq!(runs_panorama(&live, "panel"), None);
    for (name, procs, want) in [
        (
            "restored",
            one(&["/usr/bin/zsh"], 73505),
            Shell::Plain(73505),
        ),
        ("login", one(&["-zsh"], 73505), Shell::Plain(73505)),
        (
            "login-flags",
            one(&["bash", "-l", "--login", "-i"], 73505),
            Shell::Plain(73505),
        ),
        ("busy", one(&["vim", "notes"], 73600), Shell::Busy),
        ("execed", one(&["vim", "notes"], 73505), Shell::Busy),
        (
            "with-child",
            json!([{"argv": ["/usr/bin/zsh"], "pid": 73505}, {"argv": ["sleep", "9"], "pid": 73700}]),
            Shell::Busy,
        ),
        ("nested-shell", one(&["bash"], 73800), Shell::Busy),
        (
            "exec-script",
            one(&["bash", "deploy.sh"], 73505),
            Shell::Busy,
        ),
        (
            "exec-dash-c",
            one(&["zsh", "-c", "make"], 73505),
            Shell::Busy,
        ),
        ("no-argv", one(&[], 73505), Shell::Busy),
        ("unknown", json!([]), Shell::Unknown),
        ("no-list", Value::Null, Shell::Unknown),
    ] {
        assert_eq!(plain_shell(&info(procs)), want, "{name}");
    }
    let no_shell: ProcessInfo =
        serde_json::from_value(json!({"foreground_processes": one(&["zsh"], 1)})).unwrap();
    assert_eq!(plain_shell(&no_shell), Shell::Unknown);
}
