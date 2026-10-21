"""herdr-plugin.toml and pyproject agree with the code."""

import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


def VIEW_COMMAND(cmd):  # noqa: N802 - the manifest's command for the view's `cmd`
    return ["sh", "-c", f'exec sh "${{HERDR_PLUGIN_ROOT:-.}}/scripts/tui" {cmd}']


def test_plugin_manifest():
    m = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
    assert m["id"] == "leavemyyard.herdr-panorama"
    pane = m["panes"][0]
    assert pane["id"] == "panorama" and pane["placement"] == "popup"
    assert pane["command"] == VIEW_COMMAND("tui")  # scripts/tui: the Rust app or the Python one
    actions = {a["id"]: a for a in m["actions"]}
    assert set(actions) == {"open", "open-popup", "open-split"}
    assert actions["open-popup"]["command"][-2:] == ["open", "--popup"]
    assert m["startup"][0]["command"][-2:] == ["home", "--startup"]
    assert actions["open-split"]["command"][-2:] == ["open", "--split"]
    assert m["build"][0]["command"] == ["uv", "sync", "--frozen", "--no-dev"]
    form = next(p for p in m["panes"] if p["id"] == "new-agent")  # + New agent as its own popup
    assert form["placement"] == "popup" and form["command"] == VIEW_COMMAND("new-agent")
    assert isinstance(form["width"], int) and form["width"] >= 80 and form["height"] >= 24
    from herdr_panorama import herdr as hp
    assert (form["width"], form["height"]) == hp.NEW_AGENT_POPUP  # what the Python opener asks for
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


# The two launchers that probe the binary the same way: scripts/panel (the panel contract, a bare
# number) and scripts/tui (the view contract, `tui <n>`), each with the command it runs.
LAUNCHERS = {"panel": ("panel", "1"), "tui": ("tui", "tui 1")}


def _launch(root, env_extra=None, timeout=10, stdin=None, launcher="panel", path=None):
    """Runs the manifest's command for `launcher` (herdr.PANEL_COMMAND, or the view's) for a
    plugin root whose stubs echo who ran; (stdout, stderr). The probe's temp files must all be
    gone afterwards (#78 final pass, L-2)."""
    import os
    import subprocess

    from herdr_panorama import herdr
    command = herdr.PANEL_COMMAND if launcher == "panel" else VIEW_COMMAND("tui")
    env = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_") and k != "CARGO_TARGET_DIR"}
    env.update({"HERDR_PLUGIN_ROOT": str(root), "HERDR_PLUGIN_STATE_DIR": str(root / "state"),
                "PATH": path or f"{root / 'bin'}:/usr/bin:/bin", "TMPDIR": str(root / "tmp")}, **(env_extra or {}))
    (root / "state").mkdir(exist_ok=True)
    (root / "tmp").mkdir(exist_ok=True)
    (root / "scripts").mkdir(exist_ok=True)  # the plugin root holds this checkout's launcher
    (root / "scripts" / launcher).write_text((ROOT / "scripts" / launcher).read_text())
    out = subprocess.run(command, env=env, capture_output=True, text=True, timeout=timeout,
                         check=True, stdin=stdin)
    assert list((root / "tmp").iterdir()) == [], "the probe left its temp file"
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


def test_a_shared_cargo_target_dir_is_not_looked_at(tmp_path):
    """#78 review M-2: CARGO_TARGET_DIR is often shared by every checkout; its build may be
    another worktree's. Only the plugin root's own target/release counts."""
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "elsewhere" / "release" / "panorama", RUST_OK.replace("echo rust", "echo other"))
    assert _launch(tmp_path, {"CARGO_TARGET_DIR": str(tmp_path / "elsewhere")})[0] == "venv panel"
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    assert _launch(tmp_path, {"CARGO_TARGET_DIR": str(tmp_path / "elsewhere")})[0] == "rust panel"


@pytest.mark.parametrize("case,body", [
    # #78 M-1: a build from before `panel` (main.rs printed its version for anything)
    ("stale", 'echo "panorama 0.1.0"'),
    ("another contract", 'if [ "$1" = contract ]; then echo 2; exit 0; fi; echo rust "$@"'),
    # #78 final pass L-3: the compare is exact, not a prefix ("11", "1.5" are not contract 1)
    ("contract 11", 'if [ "$1" = contract ]; then echo 11; exit 0; fi; echo rust "$@"'),
    ("contract 1.5", 'if [ "$1" = contract ]; then echo 1.5; exit 0; fi; echo rust "$@"'),
    ("panics at start", 'echo "thread main panicked" >&2; exit 101'),
    ("cannot exec", "exit 126"),
], ids=["stale", "another-contract", "contract-11", "contract-1.5", "panics", "cannot-exec"])
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


def _bounded(tmp_path, body, launcher, path=None):
    import shutil
    import time
    if not (shutil.which("timeout") or shutil.which("perl")):
        pytest.skip("neither timeout(1) nor perl: the probe is unbounded here")
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", body.replace("ANSWER", LAUNCHERS[launcher][1]))
    t = time.monotonic()
    out = _launch(tmp_path, timeout=20, launcher=launcher, path=path)[0]
    return out, time.monotonic() - t


both = pytest.mark.parametrize("launcher", sorted(LAUNCHERS))


@both
def test_a_binary_that_hangs_on_the_probe_is_given_two_seconds(tmp_path, launcher):
    out, took = _bounded(tmp_path, "exec sleep 30", launcher)
    assert out == f"venv {launcher}" and took < 6


@both
def test_a_probe_that_ignores_term_is_killed_a_second_later(tmp_path, launcher):
    # #78 review L-3: `timeout 2` sent TERM only
    out, took = _bounded(tmp_path, "trap '' TERM; sleep 30", launcher)
    assert out == f"venv {launcher}" and took < 6


@both
def test_without_timeout_perls_alarm_bounds_the_probe(tmp_path, launcher):
    """A system without timeout(1) (stock macOS): perl's alarm bounds the probe instead."""
    import os
    import shutil
    if not shutil.which("perl"):
        pytest.skip("no perl")
    bin_ = tmp_path / "nobin"
    bin_.mkdir()
    for tool in ("sh", "perl", "mktemp", "head", "rm", "date", "sleep", "cat"):
        os.symlink(shutil.which(tool), bin_ / tool)  # everything the launcher uses but timeout
    out, took = _bounded(tmp_path, "exec sleep 30", launcher, path=str(bin_))
    assert out == f"venv {launcher}" and took < 6
    out, took = _bounded(tmp_path, 'if [ "$1" = contract ]; then echo "ANSWER"; exit 0; fi; echo rust "$@"',
                         launcher, path=str(bin_))
    assert out == f"rust {launcher}", "a binary that answers still passes"


@both
def test_a_probe_child_holding_the_output_cannot_keep_the_panel_waiting(tmp_path, launcher):
    # #78 review L-3: a child left behind with the probe's stdout no longer holds the read
    body = 'if [ "$1" = contract ]; then sleep 30 & echo "ANSWER"; exit 0; fi; echo rust "$@"'
    out, took = _bounded(tmp_path, body, launcher)
    assert out == f"rust {launcher}" and took < 6


@both
def test_the_probe_never_reads_the_panes_input(tmp_path, launcher):
    """#78 final pass N-8: the probe's stdin is /dev/null, so a binary that reads it can never
    take what the operator types into a new panel (here: an open, silent stdin)."""
    import os
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama",
          f'if [ "$1" = contract ]; then read x; echo "{LAUNCHERS[launcher][1]}"; exit 0; fi; echo rust "$@"')
    r, w = os.pipe()  # the write end stays open: a probe reading it would wait out its 2 s
    try:
        assert _launch(tmp_path, stdin=r, launcher=launcher)[0] == f"rust {launcher}"
    finally:
        os.close(r)
        os.close(w)


def test_a_start_that_runs_rust_clears_the_last_fallback(tmp_path):
    """#78 final pass N-9: panel-launch.log says the last fallback, not an old one."""
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", 'echo "panorama 0.1.0"')
    assert _launch(tmp_path)[0] == "venv panel"
    assert (tmp_path / "state" / "panel-launch.log").exists()
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    assert _launch(tmp_path)[0] == "rust panel"
    assert not (tmp_path / "state" / "panel-launch.log").exists()
