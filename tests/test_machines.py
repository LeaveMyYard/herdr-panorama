"""Remote machines: saved profiles and the config filter, the forwarded snapshot, the
transcript mirror command, namespacing, merging (stale, unreachable), and the view's actions
on a remote orchestrator. No ssh and no remote herdr: fakes only."""

import itertools
import json
import os
import time

import pytest

from herdr_panorama import machines
from herdr_panorama.model import Orchestrator, Swarm

_stamp = itertools.count(2_000_000)

M = machines.Machine("abc123", "devbox", "devbox")


def configure(text):
    d = os.environ["HERDR_PLUGIN_CONFIG_DIR"]
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, "config.toml")
    with open(path, "w") as fh:
        fh.write(text)
    n = next(_stamp)
    os.utime(path, (n, n))


@pytest.fixture
def profiles(monkeypatch):
    rows = [{"id": "abc123", "label": "devbox", "target": "devbox", "session": "default", "enabled": True},
            {"id": "def456", "label": "lab", "target": "lab", "session": "default", "enabled": True},
            {"id": "off789", "label": "old", "target": "old", "session": "default", "enabled": False}]
    monkeypatch.setattr(machines.herdr, "run_raw",
                        lambda *a, **k: (json.dumps(rows), None) if a[:3] == ("machine", "list", "--json") else (None, "x"))


def test_saved_machines_follow_the_config(profiles):
    configure("")
    assert [m.label for m in machines.saved()] == ["devbox", "lab"]  # default: all enabled ones
    configure('machines = ["lab", "old"]\n')
    assert [m.label for m in machines.saved()] == ["lab"]  # a disabled profile stays off
    configure('machines = "none"\n')
    assert machines.saved() == []


def test_the_snapshot_is_forwarded_read_only(monkeypatch):
    calls = []

    def run_raw(*a, **k):
        calls.append(a)
        return json.dumps({"result": {"snapshot": {"agents": [], "panes": []}}}), None
    monkeypatch.setattr(machines.herdr, "run_raw", run_raw)
    assert machines.remote_snapshot(M) == ({"agents": [], "panes": []}, None)
    assert calls == [("--machine", "abc123", "api", "snapshot")]


def test_sync_mirrors_over_batch_ssh_in_two_passes(monkeypatch, tmp_path):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    seen = []
    monkeypatch.setattr(machines, "_run", lambda argv, timeout: seen.append(argv))
    monkeypatch.setattr(machines, "append_supported", lambda: True)
    assert machines.sync(M, ["11111111-2222-3333-4444-555555555555"]) is None
    assert len(seen) == 2  # .jsonl append-only, then .json normally
    jsonl, js = seen
    assert "--append-verify" in jsonl and "--append-verify" not in js
    for argv in seen:
        assert argv[0] == "rsync" and argv[-2] == "devbox:.claude/projects/" and "--max-size=64M" in argv
        ssh = argv[argv.index("-e") + 1]
        assert "BatchMode=yes" in ssh and "ConnectTimeout=5" in ssh and "ControlMaster=auto" in ssh
        assert argv[-1].startswith(str(tmp_path / "state" / "remote"))
    assert machines.sync(M, []) is None and len(seen) == 2  # nothing to mirror: no ssh at all


SID = "11111111-2222-3333-4444-555555555555"


def remote_tree(root):
    """A session as Claude Code lays it out, plus files the scanner never reads."""
    s = root / "proj" / SID
    (s / "subagents" / "workflows" / "run1").mkdir(parents=True)
    (s / "workflows").mkdir()
    (s / "tool-results").mkdir()
    files = {f"proj/{SID}.jsonl": "a\n", f"proj/{SID}/subagents/agent-x.jsonl": "x\n",
             f"proj/{SID}/subagents/agent-x.meta.json": '{"status": "running"}',
             f"proj/{SID}/subagents/workflows/run1/journal.jsonl": "j\n",
             f"proj/{SID}/subagents/workflows/run1/agent-y.jsonl": "y\n",
             f"proj/{SID}/subagents/workflows/run1/agent-y.meta.json": "{}",
             f"proj/{SID}/workflows/run1.json": '{"status": "running"}',
             f"proj/{SID}/tool-results/big.txt": "never read",
             f"proj/{SID}/notes.md": "never read",
             "proj/99999999-0000-0000-0000-000000000000.jsonl": "another session"}
    for rel, text in files.items():
        (root / rel).write_text(text)
    return files


def mirrored(dest):
    return sorted(str(p.relative_to(dest)) for p in dest.rglob("*") if p.is_file() and p.name != "seen.json")


def test_mirror_copies_only_what_the_scanner_reads(tmp_path):
    src, dest = tmp_path / "remote", tmp_path / "mirror"
    remote_tree(src)
    dest.mkdir()
    assert machines.mirror(f"{src}/", dest, [SID]) is None
    assert mirrored(dest) == sorted([
        f"proj/{SID}.jsonl", f"proj/{SID}/subagents/agent-x.jsonl", f"proj/{SID}/subagents/agent-x.meta.json",
        f"proj/{SID}/subagents/workflows/run1/agent-y.jsonl", f"proj/{SID}/subagents/workflows/run1/agent-y.meta.json",
        f"proj/{SID}/subagents/workflows/run1/journal.jsonl", f"proj/{SID}/workflows/run1.json"])


def test_mirror_follows_appends_and_in_place_rewrites(tmp_path):
    src, dest = tmp_path / "remote", tmp_path / "mirror"
    remote_tree(src)
    dest.mkdir()
    machines.mirror(f"{src}/", dest, [SID])
    t = src / "proj" / f"{SID}.jsonl"
    t.write_text("a\nb\n")  # appended
    meta = src / "proj" / SID / "subagents" / "agent-x.meta.json"
    meta.write_text("{}")  # rewritten shorter
    summary = src / "proj" / SID / "workflows" / "run1.json"
    summary.write_text('{"status": "done!!"}')  # rewritten at the same size
    later = summary.stat().st_mtime + 5
    os.utime(summary, (later, later))
    machines.mirror(f"{src}/", dest, [SID])
    assert (dest / "proj" / f"{SID}.jsonl").read_text() == "a\nb\n"
    assert (dest / "proj" / SID / "subagents" / "agent-x.meta.json").read_text() == "{}"
    assert (dest / "proj" / SID / "workflows" / "run1.json").read_text() == '{"status": "done!!"}'


def test_mirror_caps_the_file_size(tmp_path, monkeypatch):
    src, dest = tmp_path / "remote", tmp_path / "mirror"
    remote_tree(src)
    (src / "proj" / f"{SID}.jsonl").write_text("x" * 5000)
    dest.mkdir()
    monkeypatch.setattr(machines, "MAX_FILE", "1K")
    machines.mirror(f"{src}/", dest, [SID])
    assert not (dest / "proj" / f"{SID}.jsonl").exists()


def test_prune_drops_sessions_gone_longer_than_the_done_window(tmp_path):
    dest = tmp_path / "mirror"
    (dest / "proj" / SID / "subagents").mkdir(parents=True)
    (dest / "proj" / f"{SID}.jsonl").write_text("a")
    (dest / "proj" / SID / "subagents" / "agent-x.jsonl").write_text("x")
    assert machines.prune(dest, [SID], 3600, now=1000.0) == []  # named now
    assert machines.prune(dest, [], 3600, now=1000.0 + 3000) == []  # gone, but within the window
    assert machines.prune(dest, [], 3600, now=1000.0 + 4000) == [SID]
    assert not (dest / "proj" / f"{SID}.jsonl").exists() and not (dest / "proj" / SID).exists()


def test_a_timeout_kills_rsync_and_its_children(tmp_path):
    import time as _t
    pidfile = tmp_path / "child"
    start = _t.monotonic()
    err = machines._run(["sh", "-c", f"sleep 30 & echo $! > {pidfile}; wait"], timeout=0.5)
    assert err == "rsync timed out" and _t.monotonic() - start < 3
    child = int(pidfile.read_text())
    _t.sleep(0.2)
    with pytest.raises(ProcessLookupError):
        os.kill(child, 0)  # the backgrounded child went with the group


def test_unsafe_ssh_targets_are_never_used(monkeypatch):
    rows = [{"id": "a", "label": "ok", "target": "devbox", "enabled": True},
            {"id": "b", "label": "opt", "target": "-oProxyCommand=x", "enabled": True},
            {"id": "c", "label": "space", "target": "dev box", "enabled": True}]
    monkeypatch.setattr(machines.herdr, "run_raw", lambda *a, **k: (json.dumps(rows), None))
    configure("")
    assert [m.label for m in machines.saved()] == ["ok"]
    assert machines.sync(machines.Machine("b", "opt", "-oProxyCommand=x"), [SID]) == "unsafe ssh target"


def test_namespacing_keeps_machines_apart():
    sw = Swarm(orchestrators=[Orchestrator(pane_id="w1:p1", tab_id="w1:t1", tab_label="a", session_id="s",
                                           inbox=[3])])
    sw.docked = "w1:p1"
    machines.namespace(sw, M)
    o = sw.orchestrators[0]
    assert (o.pane_id, o.tab_id, o.machine, o.machine_id, o.inbox) == ("abc123:w1:p1", "abc123:w1:t1", "devbox",
                                                                       "abc123", [])
    assert sw.docked is None  # never docked into a remote server


def test_merge_marks_stale_and_unreachable(monkeypatch):
    up = machines.Watcher(M)
    up.result = machines.Result(Swarm(orchestrators=[Orchestrator("devbox:w1:p1", "devbox:w1:t1", "a", None)]),
                                time.monotonic(), None, True)
    down = machines.Watcher(machines.Machine("x", "lab", "lab"))
    down.result = machines.Result(None, 0.0, "ssh: connect timed out", True)
    r = machines.Remotes([up, down], machines.selection())
    local = Swarm(orchestrators=[Orchestrator("w1:p1", "w1:t1", "local", None)])
    merged = r.merge(local)
    assert [o.pane_id for o in merged.orchestrators] == ["w1:p1", "devbox:w1:p1"]
    assert merged.errors == ["lab: unreachable (ssh: connect timed out)"]
    up.result = machines.Result(up.result.swarm, up.result.at - 60, None, True)  # no answer for a minute
    assert "devbox: stale (no answer)" in r.merge(Swarm()).errors


def test_a_failed_refresh_keeps_the_last_result(monkeypatch, tmp_path):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    w = machines.Watcher(M)
    snap = {"tabs": [{"tab_id": "w1:t1", "label": "work", "workspace_id": "w1"}],
            "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude"}]}
    monkeypatch.setattr(machines, "remote_snapshot", lambda m: (snap, None))
    monkeypatch.setattr(machines, "sync", lambda m, s: None)
    w.refresh()
    assert [o.pane_id for o in w.result.swarm.orchestrators] == ["abc123:w1:p1"]
    monkeypatch.setattr(machines, "remote_snapshot", lambda m: (None, "unreachable"))
    w.refresh()
    assert w.result.swarm.orchestrators[0].pane_id == "abc123:w1:p1" and w.result.error == "unreachable"


# -- the view ----------------------------------------------------------------------------

def mixed_swarm():
    from .demo import demo_swarm
    sw = demo_swarm()
    remote = demo_swarm().orchestrators[0]
    remote.pane_id, remote.tab_id = "abc123:w1:p1", "abc123:w1:t1"
    remote.machine, remote.machine_id = "devbox", "abc123"
    remote.tab_label = "Remote work"
    remote.inbox, remote.inbox_texts = [], []  # as namespace() leaves it: the inbox is local
    for a in remote.agents():
        a.question_ids, a.questions = [], []
    sw.orchestrators.append(remote)
    return sw


async def test_enter_on_a_remote_orchestrator_focuses_it_there_and_says_where(fake_herdr, monkeypatch):
    """The operator's choice: herdr cannot switch this TUI to another machine, so Enter focuses
    the pane on that machine's server and says where it is. Never a dock, never a local focus."""
    from herdr_panorama import cockpit
    from herdr_panorama.tui import SwarmApp

    from .demo import NOW
    docked = []
    monkeypatch.setattr(cockpit, "dock", lambda *a, **k: (docked.append(a), (None, None))[1])
    for cockpit_on in (False, True):
        app = SwarmApp(source=mixed_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="abc123:w1:p1",
                       home_mode=True, cockpit_mode=cockpit_on)
        notes = []
        monkeypatch.setattr(app, "notify", lambda msg, notes=notes, **kw: notes.append(msg))
        async with app.run_test(size=(160, 45)) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            assert app.view.orch == "abc123:w1:p1"  # same local ids, a different identity
            app.armed_at = 0.0
            await pilot.press("a")
            await pilot.press("g")
            for _ in range(60):
                await pilot.pause(0.05)
                if len(notes) >= 2:
                    break
        assert "answering" in notes[0] and "switch herdr to devbox" in notes[1]
    remote = [c for c in fake_herdr.calls() if c[:1] == ["--machine"]]
    assert remote == [["--machine", "abc123", "agent", "focus", "w1:p1"]] * 2  # by id, not label
    assert docked == [] and fake_herdr.calls_of("agent", "focus") == []  # nothing local


def test_remote_groups_get_a_divider_and_it_is_not_clickable():
    from herdr_panorama import render
    sw = mixed_swarm()
    for w, dock in ((160, False), (70, False), (46, True)):
        v = render.View(orch="w1:p1", dock=dock)
        if dock:
            sw.docked = "w1:p1"
        scr = render.render(sw, v, w, 60)
        rows = scr.text().splitlines()
        y = next(i for i, r in enumerate(rows) if " devbox " in r)
        assert rows[y].strip().startswith("─ devbox ─"), (w, rows[y])
        span = range(0, 32) if w >= 120 else range(w)  # wide: the orchestrator column only
        assert all(scr.target_at(x, y) is None for x in span)  # empty space: nothing
        assert sum(" devbox " in r for r in rows) == 1  # one divider per machine, nothing on cards
        assert any("Remote work" in r for r in rows[y + 1:])  # the group follows its divider


def test_two_machines_with_the_same_local_ids_are_two_identities():
    from herdr_panorama import render
    sw = mixed_swarm()
    v = render.View(orch="w1:p1")
    assert render.find_orch(sw, "w1:p1").machine is None
    assert render.find_orch(sw, "abc123:w1:p1").machine == "devbox"
    assert render.nav(sw, v)[0].count("w1:p1") == 1


def test_a_custom_source_never_starts_machines():
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: Swarm(), live_refresh=False)
    assert app.remotes is None


def test_two_machines_with_one_label_stay_apart_and_colons_are_fine():
    from herdr_panorama import render
    from .demo import demo_swarm
    sw = demo_swarm()
    for mid, label in (("aaa", "lab:1"), ("bbb", "lab:1")):
        part = Swarm(orchestrators=[Orchestrator("w1:p1", "w1:t1", f"work on {mid}", None)])
        sw.orchestrators += machines.namespace(part, machines.Machine(mid, label, "host")).orchestrators
    ids = [o.pane_id for o in sw.orchestrators[-2:]]
    assert ids == ["aaa:w1:p1", "bbb:w1:p1"] and machines.local_id(ids[0], "aaa") == "w1:p1"
    rows = render.render(sw, render.View(orch="w1:p1"), 160, 70).text().splitlines()
    assert sum(" lab:1 " in r for r in rows) == 2  # one group (divider) per machine id


def test_a_changed_machines_setting_applies_without_a_restart(monkeypatch):
    started = []
    monkeypatch.setattr(machines.Watcher, "start", lambda self: started.append(self.machine.label))
    monkeypatch.setattr(machines, "saved", lambda: [] if machines.selection() == "none" else [M])
    configure("")
    r = machines.Remotes.start()
    assert [w.machine.label for w in r.watchers] == ["devbox"] and started == ["devbox"]
    old = r.watchers[0]
    r.merge(Swarm())
    assert r.watchers[0] is old  # unchanged setting: same watcher
    configure('machines = "none"\n')
    r.merge(Swarm())
    assert r.watchers == [] and old._stop.is_set()


def test_an_rsync_without_append_verify_gets_a_normal_jsonl_pass(monkeypatch, tmp_path):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    seen = []
    monkeypatch.setattr(machines, "_run", lambda argv, timeout: seen.append(argv))
    monkeypatch.setattr(machines, "append_supported", lambda: False)  # e.g. macOS openrsync
    machines.sync(M, [SID])
    assert len(seen) == 2 and not any("--append-verify" in a for a in seen)


# -- re-review of #19 -------------------------------------------------------------------------

LIVE = "aaaaaaaa-0000-0000-0000-000000000001"
OLD = "bbbbbbbb-0000-0000-0000-000000000002"


def test_prune_never_touches_live_sessions_or_leaves_the_project(tmp_path):
    """The reviewer's probe: a stray `...jsonl` and a symlink must not take live files along."""
    dest, outside = tmp_path / "mirror", tmp_path / "outside"
    proj = dest / "-p1"
    for sid in (LIVE, OLD):
        (proj / sid).mkdir(parents=True, exist_ok=True)
        (proj / f"{sid}.jsonl").write_text("x")
    (proj / "...jsonl").write_text("x")  # stem ".."
    (proj / ".jsonl").write_text("x")  # stem ""
    outside.mkdir()
    (outside / "keep").write_text("mine")
    (proj / "evil").symlink_to(outside)
    (proj / "evil.jsonl").write_text("x")
    machines.prune(dest, [LIVE, OLD], 3600, now=0.0)
    gone = machines.prune(dest, [LIVE], 3600, now=7200.0)
    assert gone == [OLD]  # only a well-formed, stale session id
    assert (proj / f"{LIVE}.jsonl").exists() and (proj / LIVE).is_dir()
    assert (outside / "keep").read_text() == "mine" and (proj / "evil").is_symlink()
    assert not (proj / f"{OLD}.jsonl").exists() and not (proj / OLD).exists()


def test_malformed_session_ids_never_reach_rsync_or_prune():
    assert machines.valid_session("11111111-2222-3333-4444-555555555555")
    for bad in ("", ".", "..", "../x", "a b", "-" * 8, "x" * 100, "zzzzzzzz-zzzz",
                "11111111-2222-3333-4444-555555555555\n", None):  # the whole string, like discovery.is_uuid
        assert not machines.valid_session(bad)
    rules = machines._rules(["", "..", "../etc", SID], "jsonl")
    assert all(SID in r for r in rules if r.startswith("--include=*/") and r != "--include=*/")
    assert machines._rules(["", ".."], "jsonl") == ["--include=*/", "--exclude=*"]


def test_a_reload_keeps_unchanged_watchers_and_never_runs_two_per_machine(monkeypatch):
    other = machines.Machine("def456", "lab", "lab")
    started, joined = [], []
    monkeypatch.setattr(machines.Watcher, "start", lambda self: started.append(self.machine.id))
    monkeypatch.setattr(machines.Watcher, "join", lambda self, timeout=None: joined.append(self.machine.id))
    sel = {"v": [M]}
    monkeypatch.setattr(machines, "saved", lambda: sel["v"])
    configure("")
    r = machines.Remotes.start()
    keep = r.watchers[0]
    configure('machines = ["devbox", "lab"]\n')
    sel["v"] = [M, other]
    r.merge(Swarm())
    assert r.watchers[0] is keep and not keep._stop.is_set()  # devbox kept running, not restarted
    assert started == ["abc123", "def456"]  # only the added one started
    configure('machines = ["lab"]\n')
    sel["v"] = [other]
    r.merge(Swarm())
    assert keep._stop.is_set() and [w.machine.id for w in r.watchers] == ["def456"]


def test_one_machine_never_syncs_twice_at_once(monkeypatch, tmp_path):
    import threading as _th
    import time as _t
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    active, peak = [0], [0]

    def slow_mirror(*a, **k):
        active[0] += 1
        peak[0] = max(peak[0], active[0])
        _t.sleep(0.2)
        active[0] -= 1
    monkeypatch.setattr(machines, "mirror", slow_mirror)
    ts = [_th.Thread(target=machines.sync, args=(M, [SID])) for _ in range(3)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    assert peak[0] == 1  # one lock per machine around mirror + prune


# -- a conversation continued in a new session (#39) ---------------------------------------

OLD, NEW = "77777777-7777-4777-8777-777777777777", "88888888-8888-4888-8888-888888888888"


def test_a_remote_continuation_is_mirrored_and_followed_in_one_refresh(monkeypatch, tmp_path):
    """herdr on the machine still names the old session; its mirrored transcript ends with
    continued-in (then bookkeeping records, as captured), so the new session is mirrored too
    and the orchestrator shows it, in the same refresh."""
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    src = tmp_path / "remote" / "proj"
    src.mkdir(parents=True)
    (src / f"{OLD}.jsonl").write_text(
        f'{{"type":"ai-title","aiTitle":"work","sessionId":"{OLD}"}}\n'
        f'{{"type":"continued-in","timestamp":"2026-10-16T18:41:47.545Z","sessionId":"{OLD}","continuedInSessionId":"{NEW}"}}\n'
        f'{{"type":"last-prompt","lastPrompt":"x","sessionId":"{OLD}"}}\n'
        f'{{"type":"cost-state","sessionId":"{OLD}","totalCostUSD":1.0}}\n')
    (src / f"{NEW}.jsonl").write_text(f'{{"type":"ai-title","aiTitle":"work, continued","sessionId":"{NEW}"}}\n')
    synced = []

    def sync(m, sessions):
        synced.append(sorted(sessions))
        dest = machines.mirror_root(m)
        dest.mkdir(parents=True, exist_ok=True)
        return machines.mirror(f"{tmp_path / 'remote'}/", dest, sessions)
    snap = {"tabs": [{"tab_id": "w1:t1", "workspace_id": "w1"}],
            "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude",
                        "agent_session": {"kind": "id", "value": OLD}}]}
    monkeypatch.setattr(machines, "remote_snapshot", lambda m: (snap, None))
    monkeypatch.setattr(machines, "sync", sync)
    w = machines.Watcher(M)
    w.refresh()
    o = w.result.swarm.orchestrators[0]
    assert o.session_id == NEW and o.tab_label == "work, continued"
    assert synced == [[OLD], sorted([OLD, NEW])]
    w.refresh()  # known now: one sync with both
    assert synced[-1] == sorted([OLD, NEW]) and len(synced) == 3


def test_prune_keeps_a_continuation_while_its_old_session_is_named(tmp_path):
    dest = tmp_path / "mirror"
    (dest / "proj").mkdir(parents=True)
    for sid in (OLD, NEW):
        (dest / "proj" / f"{sid}.jsonl").write_text("x\n")
    machines.prune(dest, [OLD, NEW], keep_seconds=60, now=1000.0)
    machines.prune(dest, [OLD, NEW], keep_seconds=60, now=5000.0)  # still named through the old one
    assert (dest / "proj" / f"{NEW}.jsonl").exists()
