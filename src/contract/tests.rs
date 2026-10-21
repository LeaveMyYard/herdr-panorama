use super::*;

#[test]
fn the_launcher_asks_for_this_contract() {
    let script = include_str!("../../scripts/panel");
    let line = script
        .lines()
        .find(|l| l.starts_with("CONTRACT="))
        .expect("scripts/panel names its contract");
    assert_eq!(line, format!("CONTRACT={PANEL}"));
}

#[test]
fn the_contract_covers_this_protocol_major() {
    // a new protocol major is a new contract: bump both together
    assert_eq!(crate::proto::PROTOCOL.split('.').next(), Some("1"));
}

#[test]
fn the_claude_side_launchers_ask_for_this_contract() {
    for script in [
        include_str!("../../hooks/run-hook.sh"),
        include_str!("../../scripts/mcp"),
    ] {
        let line = script
            .lines()
            .find(|l| l.starts_with("CLAUDE_CONTRACT="))
            .expect("names its contract");
        assert_eq!(line, format!("CLAUDE_CONTRACT={CLAUDE}"));
    }
}

#[test]
fn the_claude_answer_is_one_no_older_binary_prints() {
    assert_eq!(answer(None).as_deref(), Some("1"));
    assert_eq!(answer(Some("claude")), Some(format!("claude {CLAUDE}")));
    assert_eq!(answer(Some("bogus")), None);
    for script in [
        include_str!("../../hooks/run-hook.sh"),
        include_str!("../../scripts/mcp"),
    ] {
        assert!(script.contains(r#"[ "$got" = "claude $CLAUDE_CONTRACT" ]"#));
    }
}

#[test]
fn the_view_launcher_asks_for_this_contract_and_no_older_binary_passes() {
    let script = include_str!("../../scripts/tui");
    let line = script
        .lines()
        .find(|l| l.starts_with("TUI_CONTRACT="))
        .expect("scripts/tui names its contract");
    assert_eq!(line, format!("TUI_CONTRACT={TUI}"));
    assert_eq!(answer(Some("tui")), Some(format!("tui {TUI}")));
    assert!(script.contains(r#"[ "$got" = "tui $TUI_CONTRACT" ]"#));
    assert!(script.contains(r#""$B" contract tui"#));
}

#[test]
fn the_manifest_runs_the_view_through_its_launcher() {
    let m = include_str!("../../herdr-plugin.toml");
    for (pane, cmd) in [("panorama", "tui"), ("new-agent", "new-agent")] {
        let block = m
            .split("[[panes]]")
            .find(|b| b.contains(&format!("id = \"{pane}\"")))
            .expect("the pane");
        let want = format!(
            r#"command = ["sh", "-c", "exec sh \"${{HERDR_PLUGIN_ROOT:-.}}/scripts/tui\" {cmd}"]"#
        );
        assert!(block.contains(&want), "{pane}: {block}");
    }
    // the actions and the startup hook (PR 10b)
    for cmd in ["open", "open --popup", "open --split", "home --startup"] {
        let want = format!(
            r#"command = ["sh", "-c", "exec sh \"${{HERDR_PLUGIN_ROOT:-.}}/scripts/tui\" {cmd}"]"#
        );
        assert_eq!(m.matches(&want).count(), 1, "{cmd}");
    }
    assert!(
        !m.contains("\"panorama\", \"open\""),
        "no action runs uv directly"
    );
}
