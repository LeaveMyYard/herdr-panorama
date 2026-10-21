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
fn a_panel_runs_the_rust_viewer_then_the_venv_then_uv_run() {
    // test_panels.py::test_viewers_start_from_the_venv_not_uv_run: uv run adds a ~31 MB
    // parent per panel, so the venv's own entry point comes before it; the Rust viewer first
    let script = include_str!("../../scripts/panel");
    let at = |s: &str| script.find(s).unwrap_or_else(|| panic!("{s} missing"));
    assert!(at(r#"exec "$B" panel"#) < at(r#".venv/bin/panorama" panel"#));
    assert!(at(r#".venv/bin/panorama" panel"#) < at("exec uv run"));
    assert_eq!(
        crate::new_agent::real::PANEL_COMMAND,
        [
            "sh",
            "-c",
            r#"exec sh "${HERDR_PLUGIN_ROOT:-.}/scripts/panel""#
        ]
    );
}
