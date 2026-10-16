"""herdr-plugin.toml and pyproject agree with the code."""

import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_plugin_manifest():
    m = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
    assert m["id"] == "leavemyyard.herdr-panorama"
    pane = m["panes"][0]
    assert pane["id"] == "panorama" and pane["placement"] == "popup"
    assert "panorama tui" in " ".join(pane["command"])
    actions = {a["id"]: a for a in m["actions"]}
    assert set(actions) == {"open", "open-popup", "open-split"}
    assert actions["open-popup"]["command"][-2:] == ["open", "--popup"]
    assert m["startup"][0]["command"][-2:] == ["home", "--startup"]
    assert actions["open-split"]["command"][-2:] == ["open", "--split"]
    assert m["build"][0]["command"] == ["uv", "sync", "--frozen", "--no-dev"]
    form = next(p for p in m["panes"] if p["id"] == "new-agent")  # + New agent as its own popup
    assert form["placement"] == "popup" and "panorama new-agent" in " ".join(form["command"])
    assert isinstance(form["width"], int) and form["width"] >= 80 and form["height"] >= 24


def test_cli_entry_point():
    p = tomllib.loads((ROOT / "pyproject.toml").read_text())
    assert p["project"]["scripts"] == {"panorama": "herdr_panorama.cli:main"}


def test_claude_plugin_manifests():
    import json
    plugin = json.loads((ROOT / ".claude-plugin/plugin.json").read_text())
    market = json.loads((ROOT / ".claude-plugin/marketplace.json").read_text())
    assert plugin["name"] == market["plugins"][0]["name"] == "herdr-panorama"
    assert plugin["version"] == market["plugins"][0]["version"]
    server = plugin["mcpServers"]["herdr-panorama"]
    assert server["env"] == {"UV_PROJECT_ENVIRONMENT": "${CLAUDE_PLUGIN_DATA}/venv-" + plugin["version"]}
    assert server["args"][-1] == "mcp" and "${CLAUDE_PLUGIN_ROOT}" in server["args"]
    hooks = json.loads((ROOT / "hooks/hooks.json").read_text())["hooks"]
    kinds = {e: h[0]["hooks"][0]["args"][1].split('"$0" ')[1].split()[0] for e, h in hooks.items()}
    assert kinds == {"SessionStart": "session-start", "SubagentStart": "subagent-start", "SubagentStop": "subagent-stop"}
    for h in hooks.values():
        spec = h[0]["hooks"][0]
        assert spec["command"] == "sh" and spec["args"][1].rstrip().endswith("exit 0")
        assert spec["args"][2] == "${CLAUDE_PLUGIN_ROOT}/src/herdr_panorama/hooks.py"
        assert spec["timeout"] <= 10
