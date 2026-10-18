//! The Claude Code side's launchers: `hooks/hooks.json` → `hooks/run-hook.sh`, and
//! `.claude-plugin/plugin.json` → `scripts/mcp`. Each runs the Rust binary when the plugin root
//! has it built and it speaks the Claude-side contract, else Python, as before. A plugin root
//! is built in a temp dir; the environment is cleared (never the session's herdr).

use serde_json::Value;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn write_exe(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A plugin root with this checkout's manifests and launchers, and a bin dir with `python3`
/// and `uv` stubs that say they ran.
fn plugin(dir: &Path) -> (PathBuf, PathBuf) {
    let root = dir.join("root");
    for rel in [
        "hooks/hooks.json",
        "hooks/run-hook.sh",
        "scripts/mcp",
        ".claude-plugin/plugin.json",
    ] {
        std::fs::create_dir_all(root.join(rel).parent().unwrap()).unwrap();
        std::fs::copy(Path::new(ROOT).join(rel), root.join(rel)).unwrap();
    }
    let bin = dir.join("bin");
    write_exe(&bin.join("python3"), "echo \"python3 $*\"");
    write_exe(&bin.join("uv"), "echo \"uv $*\"");
    (root, bin)
}

fn run(argv: &[String], dir: &Path, bin: &Path, stdin: &str) -> Output {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", dir)
        .env("PANORAMA_HOOKS_DIR", dir.join("hooks-out"))
        .env("HERDR_PANE_ID", "w1:p1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// A hook as `hooks.json` declares it, for this root.
fn hook_argv(root: &Path, event: &str) -> Vec<String> {
    let spec: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("hooks/hooks.json")).unwrap())
            .unwrap();
    let h = &spec["hooks"][event][0]["hooks"][0];
    let expand = |a: &Value| {
        a.as_str()
            .unwrap()
            .replace("${CLAUDE_PLUGIN_ROOT}", &root.display().to_string())
    };
    std::iter::once(h["command"].as_str().unwrap().to_owned())
        .chain(h["args"].as_array().unwrap().iter().map(expand))
        .collect()
}

/// The MCP server as `plugin.json` declares it, for this root.
fn mcp_argv(root: &Path) -> Vec<String> {
    let spec: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    let s = &spec["mcpServers"]["herdr-panorama"];
    let expand = |a: &Value| {
        a.as_str()
            .unwrap()
            .replace("${CLAUDE_PLUGIN_ROOT}", &root.display().to_string())
    };
    std::iter::once(s["command"].as_str().unwrap().to_owned())
        .chain(s["args"].as_array().unwrap().iter().map(expand))
        .collect()
}

const STOP: &str = r#"{"session_id": "s1", "agent_id": "a1", "agent_type": "Explore"}"#;

#[test]
fn without_a_binary_the_python_hooks_and_server_run() {
    let dir = tempfile::tempdir().unwrap();
    let (root, bin) = plugin(dir.path());
    // hooks.json discards the hook's output for SubagentStop: the stub's record is its stdout
    // for SessionStart, which hooks.json passes through
    let out = run(&hook_argv(&root, "SessionStart"), dir.path(), &bin, "{}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with("python3 -I ") && text.contains("hooks.py session-start"),
        "{text}"
    );
    assert_eq!(out.status.code(), Some(0));
    let out = run(&mcp_argv(&root), dir.path(), &bin, "");
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        text.trim(),
        format!(
            "uv run --quiet --frozen --no-dev --project {} panorama mcp",
            root.display()
        )
    );
}

#[test]
fn a_binary_with_another_contract_is_passed_over() {
    let dir = tempfile::tempdir().unwrap();
    let (root, bin) = plugin(dir.path());
    write_exe(
        &root.join("target/release/panorama"),
        "[ \"$1\" = contract ] && echo 2 && exit 0\necho \"rust $*\"",
    );
    let out = run(&hook_argv(&root, "SessionStart"), dir.path(), &bin, "{}");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("python3 ")
    );
    let out = run(&mcp_argv(&root), dir.path(), &bin, "");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("uv run ")
    );
}

#[test]
fn the_built_binary_runs_the_hooks_and_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let (root, bin) = plugin(dir.path());
    std::fs::create_dir_all(root.join("target/release")).unwrap();
    symlink(
        env!("CARGO_BIN_EXE_panorama"),
        root.join("target/release/panorama"),
    )
    .unwrap();
    for event in ["SubagentStart", "SubagentStop"] {
        let out = run(&hook_argv(&root, event), dir.path(), &bin, STOP);
        assert_eq!(
            (out.status.code(), out.stdout.as_slice()),
            (Some(0), &b""[..])
        );
    }
    let recs = std::fs::read_to_string(dir.path().join("hooks-out/s1.jsonl")).unwrap();
    let events: Vec<String> = recs
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["event"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(events, ["start", "stop"]);
    let out = run(
        &hook_argv(&root, "SessionStart"),
        dir.path(),
        &bin,
        r#"{"session_id": "s1", "source": "startup"}"#,
    );
    let said: Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert_eq!(said["hookSpecificOutput"]["hookEventName"], "SessionStart");
    // garbage in, nothing out, exit 0 (exit 2 would keep a subagent running)
    let out = run(&hook_argv(&root, "SubagentStop"), dir.path(), &bin, "{bad");
    assert_eq!(
        (
            out.status.code(),
            out.stdout.as_slice(),
            out.stderr.as_slice()
        ),
        (Some(0), &b""[..], &b""[..])
    );
    // the MCP server answers ping
    let req = "{\"jsonrpc\": \"2.0\", \"id\": 0, \"method\": \"initialize\", \"params\": {\"protocolVersion\": \"2025-06-18\", \"capabilities\": {}, \"clientInfo\": {\"name\": \"t\", \"version\": \"0\"}}}\n";
    let out = run(&mcp_argv(&root), dir.path(), &bin, req);
    let first: Value = serde_json::from_str(
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["result"]["serverInfo"]["name"], "herdr-panorama");
}

#[test]
fn a_probe_that_hangs_or_fails_or_a_directory_falls_back_to_python() {
    let dir = tempfile::tempdir().unwrap();
    let (root, bin) = plugin(dir.path());
    let exe = root.join("target/release/panorama");
    for body in ["exec sleep 30", "exit 101", "echo 1x"] {
        write_exe(&exe, body);
        let start = std::time::Instant::now();
        let out = run(&hook_argv(&root, "SessionStart"), dir.path(), &bin, "{}");
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .starts_with("python3 "),
            "{body}"
        );
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "{body}: {:?}",
            start.elapsed()
        );
        let out = run(&mcp_argv(&root), dir.path(), &bin, "");
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .starts_with("uv run "),
            "{body}"
        );
    }
    std::fs::remove_file(&exe).unwrap();
    std::fs::create_dir(&exe).unwrap();
    let out = run(&hook_argv(&root, "SessionStart"), dir.path(), &bin, "{}");
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .starts_with("python3 ")
    );
}

#[test]
fn a_binary_from_before_the_claude_contract_is_passed_over() {
    let dir = tempfile::tempdir().unwrap();
    let (root, bin) = plugin(dir.path());
    let exe = root.join("target/release/panorama");
    // one that ignores its argument and prints 1, and the operator's binary from before this
    // contract exactly: `contract` (any argument) prints the panel's 1, anything unknown prints
    // its version and exits 0
    for body in [
        "echo 1",
        "case \"$1\" in contract) echo 1 ;; panel) exit 0 ;; *) echo \"panorama 0.1.0\" ;; esac",
    ] {
        write_exe(&exe, body);
        let out = run(&hook_argv(&root, "SessionStart"), dir.path(), &bin, "{}");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            text.starts_with("python3 ") && !text.contains("panorama 0.1.0"),
            "{body}: {text}"
        );
        let out = run(&hook_argv(&root, "SubagentStart"), dir.path(), &bin, STOP);
        assert_eq!(out.status.code(), Some(0));
        let out = run(&mcp_argv(&root), dir.path(), &bin, "");
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .starts_with("uv run "),
            "{body}"
        );
    }
}

#[test]
fn the_binary_answers_only_what_it_knows() {
    let ask = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
            .env_clear()
            .args(args)
            .output()
            .unwrap();
        (out.status.code(), String::from_utf8(out.stdout).unwrap())
    };
    assert_eq!(ask(&["contract"]), (Some(0), "1\n".into()));
    assert_eq!(ask(&["contract", "claude"]), (Some(0), "claude 1\n".into()));
    assert_eq!(ask(&["contract", "bogus"]), (Some(2), String::new()));
    assert_eq!(ask(&["bogus"]), (Some(2), String::new()));
    assert!(ask(&[]).1.starts_with("panorama "));
}
