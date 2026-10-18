"""herdr-plugin.toml and pyproject agree with the code."""

import tomllib
from pathlib import Path

import pytest

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
    panel = next(p for p in m["panes"] if p["id"] == "panel")  # a per-tab panel
    from herdr_panorama import herdr
    assert panel["placement"] == "split" and panel["command"] == herdr.PANEL_COMMAND  # layout.apply uses it too


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
    kinds = {e: h[0]["hooks"][0]["args"][1].split('"$1" ')[1].split()[0] for e, h in hooks.items()}
    assert kinds == {"SessionStart": "session-start", "SubagentStart": "subagent-start", "SubagentStop": "subagent-stop"}
    for h in hooks.values():
        spec = h[0]["hooks"][0]
        assert spec["command"] == "sh" and spec["args"][1].rstrip().endswith("exit 0")
        assert spec["args"][2] == "${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.sh"
        assert spec["args"][3] == "${CLAUDE_PLUGIN_ROOT}/src/herdr_panorama/hooks.py"
        assert spec["timeout"] <= 10


def _launch(root, env_extra=None, timeout=10):
    """Runs herdr.PANEL_COMMAND for a plugin root whose stubs echo who ran; (stdout, stderr)."""
    import os
    import subprocess

    from herdr_panorama import herdr
    env = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_") and k != "CARGO_TARGET_DIR"}
    env.update({"HERDR_PLUGIN_ROOT": str(root), "HERDR_PLUGIN_STATE_DIR": str(root / "state"),
                "PATH": f"{root / 'bin'}:/usr/bin:/bin"}, **(env_extra or {}))
    (root / "state").mkdir(exist_ok=True)
    (root / "scripts").mkdir(exist_ok=True)  # the plugin root holds this checkout's launcher
    (root / "scripts" / "panel").write_text((ROOT / "scripts" / "panel").read_text())
    out = subprocess.run(herdr.PANEL_COMMAND, env=env, capture_output=True, text=True, timeout=timeout, check=True)
    return out.stdout.strip(), out.stderr.strip()


def _stub(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("#!/bin/sh\n" + body + "\n")
    path.chmod(0o755)


RUST_OK = 'if [ "$1" = contract ]; then echo 1; exit 0; fi; echo rust "$@"'


def test_the_panel_runs_the_rust_viewer_when_built_else_the_venv_else_uv(tmp_path):
    """docs/plans/rust-rewrite.md §5.4: a ported entrypoint runs the Rust binary when it is built
    (and speaks this checkout's contract), the Python command otherwise."""
    _stub(tmp_path / "bin" / "uv", 'echo uv "$@"')
    assert _launch(tmp_path)[0].startswith("uv run --quiet --frozen --no-dev --project")
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    assert _launch(tmp_path)[0] == "venv panel"
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    assert _launch(tmp_path) == ("rust panel", "")
    (tmp_path / "target" / "release" / "panorama").chmod(0o644)  # not executable: not used
    assert _launch(tmp_path)[0] == "venv panel"


def test_cargo_target_dir_is_honoured_first(tmp_path):
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    _stub(tmp_path / "elsewhere" / "release" / "panorama", RUST_OK.replace("echo rust", "echo other"))
    assert _launch(tmp_path, {"CARGO_TARGET_DIR": str(tmp_path / "elsewhere")})[0] == "other panel"
    assert _launch(tmp_path)[0] == "rust panel"


@pytest.mark.parametrize("case,body", [
    # #78 M-1: a build from before `panel` (main.rs printed its version for anything)
    ("stale", 'echo "panorama 0.1.0"'),
    ("another contract", 'if [ "$1" = contract ]; then echo 2; exit 0; fi; echo rust "$@"'),
    ("panics at start", 'echo "thread main panicked" >&2; exit 101'),
    ("cannot exec", "exit 126"),
], ids=["stale", "another-contract", "panics", "cannot-exec"])
def test_a_binary_that_does_not_speak_the_contract_falls_back_to_the_venv(tmp_path, case, body):
    """The reviewer's case: such a binary used to be exec'd as the panel, died at once, and three
    deaths gave the tab up for the core's life. Now the venv's viewer runs, and the fallback is
    said once (stderr and <state dir>/panel-launch.log)."""
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", body)
    out, err = _launch(tmp_path)
    assert out == "venv panel", case
    assert "using the Python viewer" in err and "needs 1" in err
    assert "using the Python viewer" in (tmp_path / "state" / "panel-launch.log").read_text()


def test_a_directory_named_like_the_binary_is_not_run(tmp_path):
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    (tmp_path / "target" / "release" / "panorama").mkdir(parents=True)
    assert _launch(tmp_path) == ("venv panel", "")


def test_a_binary_that_hangs_on_the_probe_is_given_two_seconds(tmp_path):
    import shutil
    import time
    if not (shutil.which("timeout") or shutil.which("perl")):
        pytest.skip("neither timeout(1) nor perl: the probe is unbounded here")
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", "exec sleep 30")
    t = time.monotonic()
    assert _launch(tmp_path, timeout=20)[0] == "venv panel"
    assert time.monotonic() - t < 10
