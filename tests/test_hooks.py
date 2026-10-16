"""The opt-in hooks: fast, silent, never failing; the records the scanner reads."""

import io
import json
import os
import subprocess
import sys

from herdr_panorama import hooks


def run(kind, payload, capsys):
    rc = hooks.main(kind, io.StringIO(payload if isinstance(payload, str) else json.dumps(payload)))
    return rc, capsys.readouterr()


def test_start_and_stop_are_recorded(capsys):
    rc, out = run("subagent-start", {"session_id": "s1", "agent_id": "agent-abc", "agent_type": "Explore"}, capsys)
    assert rc == 0 and out.out == "" and out.err == ""
    run("subagent-stop", {"session_id": "s1", "agent_id": "abc", "agent_type": "Explore",
                          "agent_transcript_path": "/x", "last_assistant_message": "done",
                          "stop_hook_active": False}, capsys)
    lines = [json.loads(x) for x in hooks.hooks_path("s1").read_text().splitlines()]
    assert [(r["event"], r["agent"], r["agent_type"], r["v"]) for r in lines] == [
        ("start", "abc", "Explore", 1), ("stop", "abc", "Explore", 1)]
    assert all(isinstance(r["ts"], float) for r in lines)


def test_bad_input_never_fails_and_writes_nothing(capsys, monkeypatch, tmp_path):
    for payload in ("{bad", "[1]", "", json.dumps({"session_id": "s1"}), json.dumps({"agent_id": "a"})):
        rc, out = run("subagent-stop", payload, capsys)
        assert rc == 0 and out.out == "" and out.err == ""
    assert not hooks.hooks_path("s1").exists()
    blocker = tmp_path / "file"
    blocker.write_text("x")
    monkeypatch.setenv("PANORAMA_HOOKS_DIR", str(blocker / "sub"))  # cannot be created
    rc, out = run("subagent-start", {"session_id": "s1", "agent_id": "a"}, capsys)
    assert rc == 0 and out.out == ""


def test_session_start_text_only_inside_herdr(capsys, monkeypatch):
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    monkeypatch.delenv("HERDR_ENV", raising=False)
    assert run("session-start", {"session_id": "s0", "source": "startup"}, capsys)[1].out == ""
    monkeypatch.setenv("HERDR_PANE_ID", "w1:p1")
    out = json.loads(run("session-start", {"session_id": "s7", "source": "startup"}, capsys)[1].out)
    ctx = out["hookSpecificOutput"]["additionalContext"]
    assert out["hookSpecificOutput"]["hookEventName"] == "SessionStart"
    assert "report_thread" in ctx and len(ctx.split()) < 70  # short


def test_process_identity():
    me = hooks._stat(os.getpid())
    assert me is not None
    assert hooks.process_alive(os.getpid(), me[2]) is True
    assert hooks.process_alive(os.getpid(), "not-my-start") is False  # a reused pid
    p = subprocess.Popen([sys.executable, "-c", "pass"])
    start = hooks._stat(p.pid)[2]
    p.wait()
    assert hooks.process_alive(p.pid, start) is False


def test_cli_entry(tmp_path):
    env = dict(os.environ, PANORAMA_HOOKS_DIR=str(tmp_path))
    p = subprocess.run([sys.executable, "-m", "herdr_panorama.cli", "hook", "subagent-start"],
                       input='{"session_id":"s9","agent_id":"x1"}', capture_output=True, text=True, env=env)
    assert p.returncode == 0 and p.stdout == ""
    assert json.loads((tmp_path / "s9.jsonl").read_text())["agent"] == "x1"


# -- review of PR #3 ------------------------------------------------------------------

ROOT = __import__("pathlib").Path(__file__).resolve().parent.parent


def hook_spec(event):
    return json.loads((ROOT / "hooks/hooks.json").read_text())["hooks"][event][0]


def run_hook(event, payload, root=ROOT, env=None):
    spec = hook_spec(event)["hooks"][0]
    argv = [spec["command"], *(a.replace("${CLAUDE_PLUGIN_ROOT}", str(root)) for a in spec["args"])]
    return subprocess.run(argv, input=json.dumps(payload), capture_output=True, text=True, timeout=30,
                          env={**os.environ, **(env or {})})


def test_b1_hooks_never_use_uv_and_always_exit_0(tmp_path):
    for event in ("SubagentStart", "SubagentStop", "SessionStart"):
        args = hook_spec(event)["hooks"][0]["args"]
        assert "uv" not in args and not any(a.startswith("UV_") for a in args)
    fake = tmp_path / "bin"
    fake.mkdir()
    for name in ("uv", "python3"):
        (fake / name).write_text("#!/bin/sh\necho 'boom' >&2\nexit 2\n")
        (fake / name).chmod(0o755)
    env = {"PATH": f"{fake}:{os.environ['PATH']}", "PANORAMA_HOOKS_DIR": str(tmp_path / "h")}
    p = run_hook("SubagentStop", {"session_id": "s", "agent_id": "a1", "agent_type": "Explore"}, env=env)
    assert p.returncode == 0 and p.stdout == "" and p.stderr == ""  # exit 2 would keep the subagent running


def test_b1_concurrent_hooks_from_two_plugin_versions(tmp_path):
    import shutil
    import threading
    roots = []
    for v in ("0.1.0", "0.2.0"):
        r = tmp_path / v
        (r / "src" / "herdr_panorama").mkdir(parents=True)
        shutil.copy(ROOT / "src/herdr_panorama/hooks.py", r / "src/herdr_panorama/hooks.py")
        roots.append(r)
    env = {"PANORAMA_HOOKS_DIR": str(tmp_path / "h")}
    codes = []

    def one(i):
        p = run_hook("SubagentStop", {"session_id": "s", "agent_id": f"a{i}", "agent_type": "x"}, roots[i % 2], env)
        codes.append((p.returncode, p.stdout))

    threads = [threading.Thread(target=one, args=(i,)) for i in range(16)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    assert codes == [(0, "")] * 16
    assert len((tmp_path / "h" / "s.jsonl").read_text().splitlines()) == 16


def test_b1_stop_hook_active_is_respected(capsys):
    run("subagent-stop", {"session_id": "s1", "agent_id": "a", "stop_hook_active": True}, capsys)
    assert not hooks.hooks_path("s1").exists()


def test_b1_unknown_kind_and_direct_run_exit_0(tmp_path):
    p = subprocess.run([sys.executable, "-I", str(ROOT / "src/herdr_panorama/hooks.py"), "bogus"],
                       input="{}", capture_output=True, text=True)
    assert p.returncode == 0 and p.stdout == ""


def test_m3_subagent_hooks_skip_internal_agents():
    for event in ("SubagentStart", "SubagentStop"):
        assert hook_spec(event)["matcher"] == ".+"  # internal agents have an empty agent_type


def test_m3_old_hook_files_are_pruned(tmp_path, monkeypatch, capsys):
    import time as _t
    d = hooks.hooks_dir()
    d.mkdir(parents=True)
    old, new = d / "old.jsonl", d / "new.jsonl"
    old.write_text("{}\n")
    new.write_text("{}\n")
    os.utime(old, (_t.time() - 30 * 86400,) * 2)
    run("session-start", {"session_id": "s1", "source": "startup"}, capsys)
    assert not old.exists() and new.exists()


def test_l4_session_text_once_per_session(capsys, monkeypatch):
    monkeypatch.setenv("HERDR_PANE_ID", "w1:p1")
    first = run("session-start", {"session_id": "s1", "source": "startup"}, capsys)[1].out
    again = run("session-start", {"session_id": "s1", "source": "compact"}, capsys)[1].out
    other = run("session-start", {"session_id": "s2", "source": "startup"}, capsys)[1].out
    assert "report_thread" in first and again == "" and "report_thread" in other


def test_m2_claude_process_found_for_native_and_node_installs(monkeypatch):
    tree = {100: (90, "python3", "s1", ["python3", "-I", "hooks.py"]),
            90: (80, "sh", "s2", ["sh", "-c", "..."]),
            80: (1, "node", "s3", ["node", "/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js"])}
    monkeypatch.setattr(hooks, "_stat", lambda pid: tree[pid][:3] if pid in tree else None)
    monkeypatch.setattr(hooks, "_cmdline", lambda pid: tree[pid][3] if pid in tree else [])
    monkeypatch.setattr(os, "getppid", lambda: 100)
    assert hooks.claude_process() == (80, "s3")
    tree[80] = (1, "2.1.288", "s4", ["/home/u/.local/share/claude/versions/2.1.288"])
    assert hooks.claude_process() == (80, "s4")
    tree[80] = (1, "bash", "s5", ["bash"])
    assert hooks.claude_process() is None  # no reliable identity: no pid recorded, never `lost`


def test_m2_another_pid_namespace_is_unknown():
    me = hooks._stat(os.getpid())
    assert hooks.process_alive(999_999_999, "x", "pid:[1]") is None or hooks.own_pidns() is None
    assert hooks.process_alive(os.getpid(), me[2], hooks.own_pidns()) is True
