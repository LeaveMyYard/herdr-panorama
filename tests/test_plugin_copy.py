"""The Claude Code plugin must work from the installed copy (herdr-inbox's lesson).

Claude Code installs a plugin as a copy in ~/.claude/plugins/cache/..., git-ignored
files included but with a ``.venv`` whose interpreter links are gone. The MCP server
therefore runs ``uv run --project ${CLAUDE_PLUGIN_ROOT}`` with
``UV_PROJECT_ENVIRONMENT=${CLAUDE_PLUGIN_DATA}/venv-<version>``, and the hooks run
``hooks/run-hook.sh`` with an interpreter cached in ``${CLAUDE_PLUGIN_DATA}``. These tests build such a copy
and run the hooks and the MCP server exactly as the manifests declare them.
"""

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
pytestmark = pytest.mark.skipif(shutil.which("uv") is None, reason="needs uv")


@pytest.fixture
def installed(tmp_path):
    copy = tmp_path / "cache" / "herdr-panorama" / "herdr-panorama" / "0.1.1"
    files = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=ROOT,
                           capture_output=True, text=True, check=True).stdout.split()
    for rel in files:
        if (ROOT / rel).is_file():
            (copy / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / rel, copy / rel)
    broken = copy / ".venv"
    (broken / "bin").mkdir(parents=True)
    (broken / "pyvenv.cfg").write_text("home = /nonexistent\nversion_info = 3.12\n")
    data = tmp_path / "data" / "herdr-panorama-herdr-panorama"
    data.mkdir(parents=True)
    return copy, data


def expand(value: str, copy: Path, data: Path) -> str:
    return value.replace("${CLAUDE_PLUGIN_ROOT}", str(copy)).replace("${CLAUDE_PLUGIN_DATA}", str(data))


def plugin_env(copy, data, **extra):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("UV_", "VIRTUAL_ENV"))}
    env.update(CLAUDE_PLUGIN_ROOT=str(copy), CLAUDE_PLUGIN_DATA=str(data), **extra)
    return env


def hook_argv(copy, data, event):
    spec = json.loads((copy / "hooks/hooks.json").read_text())["hooks"][event][0]["hooks"][0]
    return [spec["command"], *(expand(a, copy, data) for a in spec["args"])]


def test_hooks_run_from_the_copy(installed, tmp_path):
    copy, data = installed
    env = plugin_env(copy, data, PANORAMA_HOOKS_DIR=str(tmp_path / "hooks"), HERDR_PANE_ID="w1:p1")
    for event, payload in (("SubagentStart", {"session_id": "s", "agent_id": "a1", "agent_type": "Explore"}),
                           ("SubagentStop", {"session_id": "s", "agent_id": "a1", "agent_type": "Explore"})):
        p = subprocess.run(hook_argv(copy, data, event), input=json.dumps(payload), capture_output=True, text=True,
                           timeout=180, env=env, cwd=copy)
        assert p.returncode == 0 and p.stdout == "", p.stderr
    events = [json.loads(x)["event"] for x in (tmp_path / "hooks" / "s.jsonl").read_text().splitlines()]
    assert events == ["start", "stop"]
    p = subprocess.run(hook_argv(copy, data, "SessionStart"), input='{"session_id":"s","source":"startup"}',
                       capture_output=True, text=True, timeout=180, env=env, cwd=copy)
    assert json.loads(p.stdout)["hookSpecificOutput"]["hookEventName"] == "SessionStart", p.stderr
    assert not (copy / ".venv" / "bin" / "python").exists()  # the copy was left alone


async def test_mcp_server_runs_from_the_copy(installed, tmp_path):
    from mcp import Client, StdioServerParameters
    copy, data = installed
    server = json.loads((copy / ".claude-plugin/plugin.json").read_text())["mcpServers"]["herdr-panorama"]
    env = plugin_env(copy, data, CLAUDE_CODE_SESSION_ID="s", PANORAMA_EVENTS_DIR=str(tmp_path / "ev"))
    env.pop("HERDR_PANE_ID", None)
    env.update({k: expand(v, copy, data) for k, v in server.get("env", {}).items()})
    params = StdioServerParameters(command=server["command"], args=[expand(a, copy, data) for a in server["args"]],
                                   env=env, cwd=str(copy))
    async with Client(params, mode="legacy", read_timeout_seconds=180) as client:
        names = {t.name for t in (await client.list_tools()).tools}
    assert names == {"report_thread"}
