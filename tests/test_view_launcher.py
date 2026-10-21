"""scripts/tui, the view's launcher (rewrite P4 PR 10): the manifest's `panorama` and
`new-agent` panes run the Rust app when this plugin root has it built and it speaks the view
contract (`panorama contract tui` -> `tui <n>`), else the venv's app, else `uv run`; the probe
is scripts/panel's (tests/test_manifest.py covers its bounds)."""

import os
import subprocess
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
RUST_OK = 'if [ "$1" = contract ]; then [ "$2" = tui ] && echo "tui 1"; exit 0; fi; echo rust "$@"'


def _stub(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("#!/bin/sh\n" + body + "\n")
    path.chmod(0o755)


def _launch(root, pane, env_extra=None, check=True):
    """Runs the manifest's command for `pane` in a plugin root whose stubs echo who ran."""
    m = tomllib.loads((ROOT / "herdr-plugin.toml").read_text())
    command = next(p for p in m["panes"] if p["id"] == pane)["command"]
    env = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_") and k != "CARGO_TARGET_DIR"}
    env.update({"HERDR_PLUGIN_ROOT": str(root), "HERDR_PLUGIN_STATE_DIR": str(root / "state"),
                "PATH": f"{root / 'bin'}:/usr/bin:/bin", "TMPDIR": str(root / "tmp")}, **(env_extra or {}))
    for d in ("state", "tmp", "scripts"):
        (root / d).mkdir(exist_ok=True)
    (root / "scripts" / "tui").write_text((ROOT / "scripts" / "tui").read_text())
    out = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20, check=check)
    assert list((root / "tmp").iterdir()) == [], "the probe left its temp file"
    return out.stdout.strip(), out.stderr.strip()


@pytest.mark.parametrize("pane,cmd", [("panorama", "tui"), ("new-agent", "new-agent")])
def test_the_view_runs_the_rust_app_when_built_else_the_venv_else_uv(tmp_path, pane, cmd):
    _stub(tmp_path / "bin" / "uv", 'echo uv "$@"')
    assert _launch(tmp_path, pane)[0] == f"uv run --quiet --frozen --no-dev --project {tmp_path} panorama {cmd}"
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    assert _launch(tmp_path, pane)[0] == f"venv {cmd}"
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    assert _launch(tmp_path, pane) == (f"rust {cmd}", "")
    (tmp_path / "target" / "release" / "panorama").chmod(0o644)  # not executable: not used
    assert _launch(tmp_path, pane)[0] == f"venv {cmd}"


@pytest.mark.parametrize("case,body", [
    # a binary from before the view contract: its panel-only `contract` answers a bare 1
    ("panel only", 'if [ "$1" = contract ]; then echo 1; exit 0; fi; echo rust "$@"'),
    ("no contract tui", 'if [ "$1" = contract ] && [ -z "$2" ]; then echo 1; fi; exit 0'),
    ("another contract", 'if [ "$1" = contract ]; then echo "tui 2"; exit 0; fi; echo rust "$@"'),
    ("contract 11", 'if [ "$1" = contract ]; then echo "tui 11"; exit 0; fi; echo rust "$@"'),
    ("stale", 'echo "panorama 0.1.0"'),
    ("panics at start", 'echo "thread main panicked" >&2; exit 101'),
], ids=["panel-only", "no-contract-tui", "another-contract", "contract-11", "stale", "panics"])
def test_a_binary_that_does_not_speak_the_view_contract_falls_back(tmp_path, case, body):
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "target" / "release" / "panorama", body)
    out, err = _launch(tmp_path, "panorama")
    assert out == "venv tui", case
    assert "using the Python app" in err and "needs tui 1" in err
    assert "using the Python app" in (tmp_path / "state" / "tui-launch.log").read_text()
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)  # rebuilt: the log goes
    assert _launch(tmp_path, "panorama")[0] == "rust tui"
    assert not (tmp_path / "state" / "tui-launch.log").exists()


def test_a_shared_cargo_target_dir_is_not_looked_at(tmp_path):
    _stub(tmp_path / ".venv" / "bin" / "panorama", 'echo venv "$@"')
    _stub(tmp_path / "elsewhere" / "release" / "panorama", RUST_OK.replace("echo rust", "echo other"))
    assert _launch(tmp_path, "panorama", {"CARGO_TARGET_DIR": str(tmp_path / "elsewhere")})[0] == "venv tui"


def test_it_runs_only_the_views_two_commands(tmp_path):
    _stub(tmp_path / "target" / "release" / "panorama", RUST_OK)
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts" / "tui").write_text((ROOT / "scripts" / "tui").read_text())
    env = {"HERDR_PLUGIN_ROOT": str(tmp_path), "PATH": "/usr/bin:/bin"}
    out = subprocess.run(["sh", str(tmp_path / "scripts" / "tui"), "panel"], env=env,
                         capture_output=True, text=True, timeout=10)
    assert out.returncode == 2 and out.stdout == "" and "not panel" in out.stderr
