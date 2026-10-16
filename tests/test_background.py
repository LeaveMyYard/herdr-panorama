"""Background work (design turn 8, "waiting"): the transcript rules, the process check and the
collected model. Every session is written from captured templates (scenario.py)."""

import os
import sys

import pytest

from herdr_panorama import bgproc
from herdr_panorama.collect import Collector
from herdr_panorama.model import BG
from herdr_panorama.scan import wire
from herdr_panorama.scan.session import BG_CAP, MAIN, Session

from .scenario import BASE, Project, snapshot

M = 60.0
SID = "66666666-6666-4666-8666-666666666666"


def session(tmp_path):
    proj = Project(tmp_path / "projects")
    return proj, proj.main(SID)


def load(proj, alive=None) -> Session:
    s = Session(SID, str(proj.dir / f"{SID}.jsonl"))
    if alive is not None:
        s.bg_alive = alive
    s.update()
    return s


def ids(launches):
    return [x.task for x in launches]


# -- the transcript ----------------------------------------------------------------------

def test_a_background_bash_is_open_until_its_notice(tmp_path):
    proj, m = session(tmp_path)
    m.prompt(BASE, "deploy and tell me")
    m.bash_background(BASE + M, "bq1", "Wait for the rollout")
    m.write()
    s = load(proj)
    assert ids(s.waiting_bg(MAIN, BASE + 10 * M)) == ["bq1"]
    assert s.bg_description(s.bg["bq1"]) == "Wait for the rollout"
    m.notify(BASE + 20 * M, "bq1", "toolu_x", "completed")
    m.write()
    assert load(proj).waiting_bg(MAIN, BASE + 21 * M) == []


def test_a_timed_out_bash_moved_to_the_background_counts_too(tmp_path):
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq2", "Run the suite", timed_out=True)
    m.write()
    assert ids(load(proj).waiting_bg(MAIN, BASE + M)) == ["bq2"]


def test_every_task_id_of_an_orphan_notice_ends_its_task(tmp_path):
    """A resumed session is told, in one notice, about every task its previous process left."""
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq3", "a")
    m.bash_background(BASE + M, "bq4", "b")
    m.bash_background(BASE + 2 * M, "bq5", "c")
    m.orphan_notice(BASE + 3 * M, ["bq3", "bq4"])
    m.write()
    assert ids(load(proj).waiting_bg(MAIN, BASE + 4 * M)) == ["bq5"]
    notes = wire.parse_notices("<task-notification><task-id>x</task-id><task-id>y</task-id>"
                               "<task-id>__orphan_summary__:shell</task-id><status>stopped</status></task-notification>")
    assert [n["task"] for n in notes] == ["x", "y"] and {n["status"] for n in notes} == {"cxl"}


def test_a_taskstop_ends_it_and_a_failed_one_does_not(tmp_path):
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq6", "a")
    m.bash_background(BASE, "bq7", "b")
    m.task_stop(BASE + M, "bq6")
    m.task_stop(BASE + M, "bq7", error=True)
    m.write()
    assert ids(load(proj).waiting_bg(MAIN, BASE + 2 * M)) == ["bq7"]


def test_a_monitor_ends_with_its_expiry_event_or_its_timeout(tmp_path):
    proj, m = session(tmp_path)
    m.monitor(BASE, "mo1", "CI events", timeout_ms=30 * 60 * 1000)
    m.monitor(BASE, "mo2", "deploy events", timeout_ms=30 * 60 * 1000)
    m.monitor(BASE, "mo3", "forever", timeout_ms=30 * 60 * 1000, persistent=True)
    m.monitor_event(BASE + 5 * M, "mo1", "run 12 in_progress")  # an event, not the end
    m.monitor_event(BASE + 10 * M, "mo2", "[Monitor expired after 30m with 2 events delivered.]")
    m.write()
    s = load(proj)
    assert ids(s.waiting_bg(MAIN, BASE + 11 * M)) == ["mo1", "mo3"]
    assert s.bg["mo1"].kind == "Monitor" and s.bg_description(s.bg["mo1"]) == "CI events"
    assert ids(s.waiting_bg(MAIN, BASE + 32 * M)) == ["mo3"]  # mo1 timed out; a persistent one does not


def test_an_unchecked_task_counts_for_at_most_bg_cap(tmp_path):
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq8", "a")
    m.write()
    s = load(proj)
    assert ids(s.waiting_bg(MAIN, BASE + BG_CAP - M)) == ["bq8"]
    assert s.waiting_bg(MAIN, BASE + BG_CAP + 2 * M) == []


def test_the_process_check_decides_when_it_can(tmp_path):
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq9", "a")
    m.bash_background(BASE, "bq10", "b")
    m.write()
    s = load(proj, alive=lambda x: x.task == "bq9")  # bq10's shell is gone without a notice
    assert ids(s.waiting_bg(MAIN, BASE + 30 * 3600)) == ["bq9"]  # a live one has no age cap


# -- agents ------------------------------------------------------------------------------

def agent(proj, aid, **meta):
    return proj.agent(SID, aid, {"description": "x", "agentType": "general-purpose", "spawnDepth": 1, **meta})


def test_an_agent_whose_turn_ended_on_background_work_is_waiting(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "Run the suite and report")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash_background(BASE + M, "ab1", "Wait for the integration suite")
    a1.text(BASE + 2 * M, "Waiting for the suite.")
    m.write(), a1.write()
    s = load(proj)
    assert s.status_of("a1", BASE + 30 * M)[0] == "bg"  # past ASYNC_QUIET, still waiting
    m.notify(BASE + 40 * M, "ab1", "toolu_x", "completed")
    m.write()
    assert load(proj).status_of("a1", BASE + 41 * M)[0] == "done"  # its task ended: silent, done?


def test_an_agent_notified_complete_is_done_whatever_it_left_behind(tmp_path):
    """The completion guard: a finished agent's leftover loops (seen alive for days on the
    operator's machine) never make it waiting."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a2", "x")
    a2 = agent(proj, "a2", toolUseId=call)
    a2.prompt(BASE, "brief")
    a2.bash_background(BASE + M, "ab2", "a poll loop")
    m.notify(BASE + 5 * M, "a2", call, "completed")
    m.write(), a2.write()
    assert load(proj, alive=lambda x: True).status_of("a2", BASE + 30 * M)[0] == "done"


def test_a_fork_does_not_inherit_its_parents_background_tasks(tmp_path):
    proj, m = session(tmp_path)
    m.bash_background(BASE, "bq11", "main's")
    call = m.launch(BASE + M, "f1", "fork")
    f1 = agent(proj, "f1", toolUseId=call)
    f1.bash_background(BASE, "bq11", "main's")  # the fork's file repeats main's history
    m.write(), f1.write()
    s = load(proj)
    assert s.bg["bq11"].owner == MAIN and ids(s.waiting_bg("f1", BASE + 2 * M)) == []


# -- the process check ---------------------------------------------------------------------

def test_task_of_reads_the_output_path():
    assert bgproc.task_of("/tmp/claude-1000/-p/sid/tasks/b1x2.output") == "b1x2"
    assert bgproc.task_of("/tmp/x/b1x2.output") is None and bgproc.task_of("/tmp/tasks/notes.txt") is None


LINUX = sys.platform.startswith("linux")


@pytest.mark.skipif(not LINUX, reason="the process check reads /proc (Linux); elsewhere it says unknown")
def test_live_tasks_finds_the_output_files_its_descendants_hold(tmp_path):
    out = tmp_path / "tasks" / "bz9.output"
    out.parent.mkdir()
    out.write_text("")
    import subprocess
    child = subprocess.Popen([sys.executable, "-c", f"import time; f=open({str(out)!r}); time.sleep(30)"])
    try:
        for _ in range(50):
            if "bz9" in (bgproc.live_tasks(os.getpid()) or set()):
                break
            import time
            time.sleep(0.05)
        assert "bz9" in bgproc.live_tasks(os.getpid())
        assert bgproc.live_tasks(child.pid) == set()  # its own children: none
    finally:
        child.kill()
        child.wait()
    assert bgproc.live_tasks(2 ** 22 + 7) is None  # no such process: unknown, not dead


# -- the model -------------------------------------------------------------------------------

def test_collect_fills_bg_last_activity_and_stale_after(tmp_path):
    proj, m = session(tmp_path)
    m.prompt(BASE, "deploy")
    m.bash_background(BASE + M, "bq12", "Wait for the rollout")
    m.text(BASE + 2 * M, "watching")
    m.write()
    snap = snapshot([{"pane": "w1:p1", "tab": "w1:t1", "label": "deploy", "status": "idle", "session": SID}])
    asked = []
    c = Collector(root=str(proj.root), now=lambda: BASE + 10 * M, done_window=3600, stale_after=7200,
                  pane_pid=lambda pane: asked.append(pane) or os.getpid())
    sw = c.collect(snap, None)
    o = sw.orchestrators[0]
    assert sw.stale_after == 7200 and o.last_activity == BASE + 2 * M
    assert asked == ["w1:p1"]
    if LINUX:
        assert o.bg == []  # our own process holds no bq12 output: dead, no notice
    else:
        assert [b.id for b in o.bg] == ["bq12"]  # no /proc: unknown, the transcript and its cap decide
    remote = Collector(root=str(proj.root), now=lambda: BASE + 10 * M, done_window=3600, procs=False,
                       pane_pid=lambda pane: asked.append("never") or None)
    o = remote.collect(snap, None).orchestrators[0]
    assert [(b.id, b.kind, b.description) for b in o.bg] == [("bq12", "Bash", "Wait for the rollout")]
    assert "never" not in asked


def test_a_waiting_agent_says_so_in_the_model(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a3", "Run the suite")
    a3 = agent(proj, "a3", toolUseId=call)
    a3.prompt(BASE, "brief")
    a3.bash_background(BASE + M, "ab3", "Wait for the suite")
    m.write(), a3.write()
    snap = snapshot([{"pane": "w1:p1", "tab": "w1:t1", "label": "x", "status": "idle", "session": SID}])
    sw = Collector(root=str(proj.root), now=lambda: BASE + 30 * M, done_window=3600, procs=False).collect(snap, None)
    ag = next(a for a in sw.orchestrators[0].agents() if a.id == "a3")
    assert ag.status == BG and [b.id for b in ag.bg] == ["ab3"]
    assert ag.activity == "turn ended · 1 task in background"
    assert sw.counts()["waiting"] == 1


def test_an_old_agent_file_is_still_read_when_its_agent_never_ended(tmp_path):
    """Review (Low): panorama starting while a subagent has been idle for longer than the done
    window, waiting on its own background Bash: its file is old, but its agent never ended."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a4", "x")
    a4 = agent(proj, "a4", toolUseId=call)
    a4.prompt(BASE, "brief")
    a4.bash_background(BASE + M, "ab4", "a long suite")
    done_call = m.launch(BASE, "a5", "y")
    a5 = agent(proj, "a5", toolUseId=done_call)
    a5.prompt(BASE, "brief")
    m.notify(BASE + 2 * M, "a5", done_call, "completed")
    m.write(), a4.write(), a5.write()
    old = BASE + 2 * M
    for f in (a4.path, a5.path):
        os.utime(f, (old, old))
    s = Session(SID, str(proj.dir / f"{SID}.jsonl"))
    s.update(since=BASE + 3 * 3600)  # both files are older than the window
    assert s.status_of("a4", BASE + 4 * 3600)[0] == "bg"
    assert s.file_of("a5").tail.identity is None  # a finished one is still skipped
