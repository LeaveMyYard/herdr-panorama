"""hooks/run-hook.sh: the hook interpreter is resolved once, cached in CLAUDE_PLUGIN_DATA, then run
directly, so a version-manager shim (pyenv's bash script, 60-75 ms) is paid at most once.

Every case must still record the event, print nothing on failure and exit 0."""

import json
import os
import shutil
import subprocess
import sys
import threading
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
REAL_PY = os.path.realpath(sys.executable)
PAYLOAD = {"session_id": "s", "agent_id": "a1", "agent_type": "Explore"}


def hook_argv(event):
    spec = json.loads((ROOT / "hooks/hooks.json").read_text())["hooks"][event][0]["hooks"][0]
    return [spec["command"], *(a.replace("${CLAUDE_PLUGIN_ROOT}", str(ROOT)) for a in spec["args"])]


def base_env(tmp_path, path, data=None):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("HERDR_", "CLAUDE_PLUGIN_", "PYENV_", "VIRTUAL_ENV"))}
    env.update(PATH=path, PANORAMA_HOOKS_DIR=str(tmp_path / "h"))
    if data is not None:
        env["CLAUDE_PLUGIN_DATA"] = str(data)
    return env


def run(env, event="SubagentStart", payload=PAYLOAD):
    return subprocess.run(hook_argv(event), input=json.dumps(payload), capture_output=True, text=True,
                          timeout=30, env=env)


def recorded(tmp_path):
    f = tmp_path / "h" / "s.jsonl"
    return [json.loads(x) for x in f.read_text().splitlines()] if f.exists() else []


def tool_bin(tmp_path, python3=None):
    """A PATH holding only what run-hook.sh needs, plus an optional ``python3``."""
    b = tmp_path / "bin"
    b.mkdir(exist_ok=True)
    for tool in ("sh", "mkdir", "mv", "rm"):
        found = shutil.which(tool)
        assert found, tool
        if not (b / tool).exists():
            (b / tool).symlink_to(found)
    if python3 is not None:
        (b / "python3").write_text(python3)
        (b / "python3").chmod(0o755)
    return str(b)


def shim(tmp_path):
    """A pyenv-like shim: a shell script that logs each call and execs the real interpreter."""
    log = tmp_path / "shim.log"
    return tool_bin(tmp_path, f'#!/bin/sh\necho call >> "{log}"\nexec "{REAL_PY}" "$@"\n'), log


def calls(log):
    return len(log.read_text().splitlines()) if log.exists() else 0


def test_cold_cache_resolves_once_and_writes_the_real_interpreter(tmp_path):
    path, log = shim(tmp_path)
    data = tmp_path / "data" / "nested"  # not created yet
    p = run(base_env(tmp_path, path, data))
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert [r["event"] for r in recorded(tmp_path)] == ["start"]
    cached = (data / "python-interpreter").read_text()
    assert cached.endswith("\n") and os.path.realpath(cached.strip()) == REAL_PY  # not the shim
    assert calls(log) == 1  # the one resolution; the hook itself ran on the cached interpreter
    assert sorted(x.name for x in data.iterdir()) == ["python-interpreter"]  # no temp file left


def test_warm_cache_never_touches_python3(tmp_path):
    path, log = shim(tmp_path)
    data = tmp_path / "data"
    env = base_env(tmp_path, path, data)
    run(env)
    for event in ("SubagentStop", "SubagentStart", "SubagentStop"):
        p = run(env, event)
        assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert calls(log) == 1
    assert [r["event"] for r in recorded(tmp_path)] == ["start", "stop", "start", "stop"]


def test_warm_cache_session_start_output_passes_through(tmp_path):
    path, log = shim(tmp_path)
    data = tmp_path / "data"
    env = base_env(tmp_path, path, data)
    env["HERDR_PANE_ID"] = "w1:p1"
    run(env)
    p = run(env, "SessionStart", {"session_id": "s", "source": "startup"})
    assert p.returncode == 0 and calls(log) == 1
    assert json.loads(p.stdout)["hookSpecificOutput"]["hookEventName"] == "SessionStart"


def test_cache_used_even_when_python3_is_broken(tmp_path):
    data = tmp_path / "data"
    data.mkdir()
    (data / "python-interpreter").write_text(REAL_PY + "\n")
    path = tool_bin(tmp_path, "#!/bin/sh\necho boom >&2\nexit 2\n")
    p = run(base_env(tmp_path, path, data))
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert [r["event"] for r in recorded(tmp_path)] == ["start"]


@pytest.mark.parametrize("stale", ["/nonexistent/bin/python3", "python3", "", "garbage\x01\n",
                                   "{dir}", "{noexec}"])
def test_stale_cache_self_heals(tmp_path, stale):
    path, log = shim(tmp_path)
    data = tmp_path / "data"
    data.mkdir()
    noexec = tmp_path / "noexec"
    noexec.write_text("#!/bin/sh\nexit 0\n")
    (data / "python-interpreter").write_text(stale.format(dir=tmp_path, noexec=noexec))
    p = run(base_env(tmp_path, path, data))
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert [r["event"] for r in recorded(tmp_path)] == ["start"]
    assert os.path.realpath((data / "python-interpreter").read_text().strip()) == REAL_PY
    run(base_env(tmp_path, path, data), "SubagentStop")
    assert calls(log) == 1  # healed: the next hook uses the cache


@pytest.mark.skipif(os.geteuid() == 0, reason="root ignores directory permissions")
def test_unwritable_state_dir_still_runs_the_hook(tmp_path):
    path, log = shim(tmp_path)
    data = tmp_path / "data"
    data.mkdir()
    data.chmod(0o500)
    try:
        for _ in range(2):
            p = run(base_env(tmp_path, path, data))
            assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
        assert list(data.iterdir()) == []
    finally:
        data.chmod(0o700)
    assert len(recorded(tmp_path)) == 2
    assert calls(log) == 2  # resolved each time (no cache), the hook itself ran on the real interpreter


def test_state_dir_path_is_a_file(tmp_path):
    path, _ = shim(tmp_path)
    data = tmp_path / "data"
    data.write_text("x")
    p = run(base_env(tmp_path, path, data))
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert len(recorded(tmp_path)) == 1 and data.read_text() == "x"


@pytest.mark.parametrize("with_data", [True, False])
def test_missing_python_is_silent_and_exits_0(tmp_path, with_data):
    data = tmp_path / "data" if with_data else None
    env = base_env(tmp_path, tool_bin(tmp_path), data)
    env["HERDR_PANE_ID"] = "w1:p1"  # SessionStart would print, if Python ran
    for event in ("SubagentStart", "SubagentStop", "SessionStart"):
        p = run(env, event)
        assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert recorded(tmp_path) == []
    assert data is None or not (data / "python-interpreter").exists()


def test_no_plugin_data_env_uses_plain_python3(tmp_path):
    path, log = shim(tmp_path)
    for event in ("SubagentStart", "SubagentStop"):
        p = run(base_env(tmp_path, path), event)
        assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert calls(log) == 2  # every hook went through python3; nothing cached anywhere
    assert [r["event"] for r in recorded(tmp_path)] == ["start", "stop"]


def test_concurrent_cold_hooks_agree_on_one_whole_cache(tmp_path):
    path, _ = shim(tmp_path)
    data = tmp_path / "data"
    env = base_env(tmp_path, path, data)
    codes = []

    def one(i):
        p = run(env, payload={**PAYLOAD, "agent_id": f"a{i}"})
        codes.append((p.returncode, p.stdout, p.stderr))

    threads = [threading.Thread(target=one, args=(i,)) for i in range(16)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    assert codes == [(0, "", "")] * 16
    assert len(recorded(tmp_path)) == 16
    assert [x.name for x in data.iterdir()] == ["python-interpreter"]
    assert os.path.realpath((data / "python-interpreter").read_text().strip()) == REAL_PY


# -- review round 1 ---------------------------------------------------------------------

def old_python(tmp_path, version="(3, 9, 18, 'final', 0)"):
    """A ``python3`` that answers the resolver as an older Python would, and runs hooks normally."""
    log = tmp_path / "old.log"
    return tool_bin(tmp_path, f'''#!/bin/sh
echo call >> "{log}"
if [ "$2" = -c ]; then
    exec "{REAL_PY}" -I -c "import sys; sys.version_info = {version}; exec(sys.argv[1])" "$3"
fi
exec "{REAL_PY}" "$@"
'''), log


def test_an_interpreter_too_old_for_the_hooks_is_never_cached(tmp_path):
    path, log = old_python(tmp_path)
    data = tmp_path / "data"
    env = base_env(tmp_path, path, data)
    for event in ("SubagentStart", "SubagentStop"):
        p = run(env, event)
        assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert not (data / "python-interpreter").exists()
    assert [r["event"] for r in recorded(tmp_path)] == ["start", "stop"]  # still ran, through plain python3
    assert calls(log) == 4  # resolver + hook, each time: nothing pinned


def test_python_3_10_is_new_enough_to_cache(tmp_path):
    path, _ = old_python(tmp_path, "(3, 10, 0, 'final', 0)")
    data = tmp_path / "data"
    assert run(base_env(tmp_path, path, data)).returncode == 0
    assert os.path.realpath((data / "python-interpreter").read_text().strip()) == REAL_PY


def session_start_env(tmp_path, data):
    path, log = shim(tmp_path)
    env = base_env(tmp_path, path, data)
    env["HERDR_PANE_ID"] = "w1:p1"
    return env


def assert_only_the_session_json(p):
    assert p.returncode == 0 and p.stderr == ""
    assert len(p.stdout.splitlines()) == 1  # nothing but hooks.py's one JSON object reaches the model
    assert json.loads(p.stdout)["hookSpecificOutput"]["hookEventName"] == "SessionStart"


def test_cold_session_start_prints_only_its_json(tmp_path):
    data = tmp_path / "data" / "nested"
    p = run(session_start_env(tmp_path, data), "SessionStart", {"session_id": "s", "source": "startup"})
    assert_only_the_session_json(p)
    assert (data / "python-interpreter").exists()


@pytest.mark.skipif(os.geteuid() == 0, reason="root ignores directory permissions")
def test_session_start_on_an_unwritable_dir_prints_only_its_json(tmp_path):
    data = tmp_path / "data"
    data.mkdir()
    data.chmod(0o500)
    try:
        p = run(session_start_env(tmp_path, data), "SessionStart", {"session_id": "s", "source": "startup"})
    finally:
        data.chmod(0o700)
    assert_only_the_session_json(p)


def test_session_start_with_data_path_a_file_prints_only_its_json(tmp_path):
    data = tmp_path / "data"
    data.write_text("x")
    p = run(session_start_env(tmp_path, data), "SessionStart", {"session_id": "s", "source": "startup"})
    assert_only_the_session_json(p)


@pytest.mark.parametrize("warm", [False, True])
def test_hooks_run_isolated_from_pythonpath(tmp_path, warm):
    poison = tmp_path / "poison"
    poison.mkdir()
    (poison / "json.py").write_text("raise SystemExit(3)\n")
    path, _ = shim(tmp_path)
    data = tmp_path / "data"
    env = base_env(tmp_path, path, data)
    if warm:
        run(env)
    env["PYTHONPATH"] = str(poison)
    p = run(env, "SubagentStop")
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert recorded(tmp_path)[-1]["event"] == "stop"  # -I: the poisoned json was never imported


def test_a_relative_cache_entry_is_never_run_even_if_it_exists_in_the_cwd(tmp_path):
    path, log = shim(tmp_path)
    cwd = tmp_path / "cwd"
    cwd.mkdir()
    trap = cwd / "python3"
    trap_log = tmp_path / "trap.log"
    trap.write_text(f'#!/bin/sh\necho call >> "{trap_log}"\nexit 0\n')
    trap.chmod(0o755)
    data = tmp_path / "data"
    data.mkdir()
    (data / "python-interpreter").write_text("python3\n")
    p = subprocess.run(hook_argv("SubagentStart"), input=json.dumps(PAYLOAD), capture_output=True, text=True,
                       timeout=30, env=base_env(tmp_path, path, data), cwd=cwd)
    assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert not trap_log.exists()
    assert [r["event"] for r in recorded(tmp_path)] == ["start"]
    assert os.path.realpath((data / "python-interpreter").read_text().strip()) == REAL_PY


def test_a_directory_in_the_cache_place_collects_no_temp_files(tmp_path):
    path, _ = shim(tmp_path)
    data = tmp_path / "data"
    (data / "python-interpreter").mkdir(parents=True)
    for event in ("SubagentStart", "SubagentStop"):
        p = run(base_env(tmp_path, path, data), event)
        assert (p.returncode, p.stdout, p.stderr) == (0, "", "")
    assert list((data / "python-interpreter").iterdir()) == []
    assert [x.name for x in data.iterdir()] == ["python-interpreter"]
    assert len(recorded(tmp_path)) == 2
