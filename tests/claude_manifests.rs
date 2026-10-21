//! The Claude Code plugin's manifests as this checkout ships them (Python
//! `test_manifest.py::test_claude_plugin_manifests`, `test_hooks.py::test_m3_…`): the plugin and
//! its marketplace entry agree, the MCP server runs `scripts/mcp … mcp`, and every hook runs
//! `run-hook.sh` for its own kind through `sh`, ending in `exit 0`, within 10 s.

use serde_json::Value;
use std::path::Path;

fn json(rel: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn hook(hooks: &Value, event: &str) -> Value {
    hooks[event][0]["hooks"][0].clone()
}

#[test]
fn the_plugin_and_its_marketplace_entry_agree_and_the_server_runs_mcp() {
    let (plugin, market) = (
        json(".claude-plugin/plugin.json"),
        json(".claude-plugin/marketplace.json"),
    );
    assert_eq!(plugin["name"], "herdr-panorama");
    assert_eq!(market["plugins"][0]["name"], plugin["name"]);
    assert_eq!(market["plugins"][0]["version"], plugin["version"]);
    let server = &plugin["mcpServers"]["herdr-panorama"];
    let venv = format!(
        "${{CLAUDE_PLUGIN_DATA}}/venv-{}",
        plugin["version"].as_str().unwrap()
    );
    assert_eq!(
        server["env"],
        serde_json::json!({ "UV_PROJECT_ENVIRONMENT": venv })
    );
    let args: Vec<&str> = server["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(args.last(), Some(&"mcp"));
    assert!(args.contains(&"${CLAUDE_PLUGIN_ROOT}"), "{args:?}");
}

#[test]
fn every_hook_runs_its_own_kind_through_sh_and_always_exits_0() {
    let hooks = json("hooks/hooks.json")["hooks"].clone();
    let mut kinds = Vec::new();
    for (event, _) in hooks.as_object().unwrap() {
        let spec = hook(&hooks, event);
        let script = spec["args"][1].as_str().unwrap();
        kinds.push((
            event.clone(),
            script
                .split("\"$1\" ")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .to_owned(),
        ));
        assert_eq!(spec["command"], "sh");
        assert!(script.trim_end().ends_with("exit 0"), "{script}");
        assert_eq!(spec["args"][2], "${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.sh");
        assert_eq!(
            spec["args"][3],
            "${CLAUDE_PLUGIN_ROOT}/src/herdr_panorama/hooks.py"
        );
        assert!(spec["timeout"].as_u64().unwrap() <= 10);
    }
    kinds.sort();
    let want = [
        ("SessionStart", "session-start"),
        ("SubagentStart", "subagent-start"),
        ("SubagentStop", "subagent-stop"),
    ];
    assert_eq!(kinds, want.map(|(e, k)| (e.to_owned(), k.to_owned())));
}

#[test]
fn subagent_hooks_skip_internal_agents() {
    // Claude Code's internal agents have an empty agent_type: `.+` never matches them
    let hooks = json("hooks/hooks.json")["hooks"].clone();
    for event in ["SubagentStart", "SubagentStop"] {
        assert_eq!(hooks[event][0]["matcher"], ".+", "{event}");
    }
}
