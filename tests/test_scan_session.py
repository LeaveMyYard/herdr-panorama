"""The fold and the status rules, one test per fix from the zoetrope study.

Every session is written from captured templates (scenario.py).
"""

import json
import os
import random

from herdr_panorama.scan.session import ACTIVE, ASYNC_QUIET, MAIN, PENDING_CAP, Session

from .scenario import BASE, Project

M = 60.0
SID = "55555555-5555-4555-8555-555555555555"


def session(tmp_path) -> tuple[Project, object]:
    proj = Project(tmp_path / "projects")
    return proj, proj.main(SID)


def load(proj) -> Session:
    s = Session(SID, str(proj.dir / f"{SID}.jsonl"))
    s.update()
    return s


def agent(proj, aid, desc="work", **meta):
    return proj.agent(SID, aid, {"description": desc, "agentType": "general-purpose", "spawnDepth": 1, **meta})


def write(*ts):
    for t in ts:
        t.write()


def test_notice_from_the_queue_carrier_ends_the_agent(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "Review PR #1")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + 2 * M, "look")
    m.notify(BASE + 3 * M, "a1", call, "completed", carriers=("queue",))  # no attachment, no user string
    write(m, a1)
    s = load(proj)
    assert s.status_of("a1", BASE + 4 * M)[0] == "done"


def test_killed_is_terminal(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + 1 * M, "work", pending=True)
    m.notify(BASE + 2 * M, "a1", call, "killed")
    write(m, a1)
    assert load(proj).status_of("a1", BASE + 3 * M)[0] == "cxl"


def test_latest_notice_wins_across_resumes(tmp_path):
    """failed -> resumed -> completed: the agent is done, not pinned to the first failure (zoetrope)."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + 1 * M, "try")
    m.notify(BASE + 2 * M, "a1", call, "failed")
    m.resume(BASE + 3 * M, "a1")
    a1.bash(BASE + 4 * M, "retry")
    m.notify(BASE + 5 * M, "a1", call, "completed")
    write(m, a1)
    s = load(proj)
    assert s.status_of("a1", BASE + 6 * M)[0] == "done"
    assert len(s.resumes["a1"]) == 1


def test_a_resume_reopens_a_finished_agent(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    m.notify(BASE + 2 * M, "a1", call, "completed")
    m.resume(BASE + 10 * M, "a1")  # resumed; the agent has not written anything yet
    write(m, a1)
    assert load(proj).status_of("a1", BASE + 10.5 * M)[0] == "run"


def test_activity_after_a_notice_reopens_the_agent(tmp_path):
    """A message queued while it was finishing resumes it without a resumedAgentId: its own lines tell."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    m.notify(BASE + 2 * M, "a1", call, "completed")
    a1.bash(BASE + 3 * M, "more work")
    write(m, a1)
    s = load(proj)
    assert s.status_of("a1", BASE + 3.5 * M)[0] == "run"
    # a final line landing a moment after its own notice does not reopen it
    proj2 = Project(tmp_path / "p2")
    m2 = proj2.main(SID)
    call = m2.launch(BASE, "a1", "x")
    b = proj2.agent(SID, "a1", {"description": "x", "toolUseId": call})
    b.prompt(BASE, "brief")
    m2.notify(BASE + 2 * M, "a1", call, "completed")
    b.handback(BASE + 2 * M + 1)
    write(m2, b)
    s2 = Session(SID, str(proj2.dir / f"{SID}.jsonl"))
    s2.update()
    assert s2.status_of("a1", BASE + 2.2 * M)[0] == "done"


def test_an_abandoned_tool_call_expires(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + 1 * M, "gh run watch", pending=True)
    write(m, a1)
    s = load(proj)
    assert s.status_of("a1", BASE + 30 * M)[0] == "run"  # long tool in flight
    status, _, inferred = s.status_of("a1", BASE + M + PENDING_CAP + 1)
    assert status == "done" and inferred  # never notified, call abandoned: ended silently


def test_background_agent_without_notice_keeps_running_through_a_long_turn(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + 1 * M, "step")
    write(m, a1)
    s = load(proj)
    assert s.status_of("a1", BASE + M + ACTIVE + 30) == ("run", None, True)
    assert s.status_of("a1", BASE + M + ASYNC_QUIET + 30)[0] == "done"


def test_foreground_agent_completes_with_its_result(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "f1", "plan", background=False)
    f1 = agent(proj, "f1", toolUseId=call, requestShape="foreground")
    f1.prompt(BASE, "brief")
    f1.bash(BASE + 1 * M, "think")
    m.foreground_done(BASE + 2 * M, call, "f1", 9)
    call2 = m.launch(BASE + 3 * M, "f2", "plan again", background=False)
    m.foreground_done(BASE + 4 * M, call2, "f2", 1, error=True)
    write(m, f1)
    s = load(proj)
    assert s.status_of("f1", BASE + 2.5 * M)[0] == "done" and s.ack("f1").tool_count == 9
    assert s.status_of("f2", BASE + 4.5 * M)[0] == "fail"


def test_nested_agents_are_parented_through_the_spawning_call(tmp_path):
    """A depth-2 agent's toolUseId lives in its parent's file; its notice arrives there too."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "Author PR #3")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    inner = a1.launch(BASE + 1 * M, "n1", "Explore call sites", model="haiku", subagent_type="Explore")
    n1 = proj.agent(SID, "n1", {"description": "Explore call sites", "spawnDepth": 2, "toolUseId": inner},
                    kind="meta_nested")
    n1.prompt(BASE + 1 * M, "brief")
    n1.bash(BASE + 2 * M, "grep")
    a1.notify(BASE + 3 * M, "n1", inner, "failed")
    write(m, a1, n1)
    s = load(proj)
    assert s.parent_of("n1") == "a1" and s.parent_of("a1") == MAIN
    assert s.status_of("n1", BASE + 4 * M)[0] == "fail"


def test_a_fork_does_not_take_over_its_own_spawning_call(tmp_path):
    """A fork's file repeats its parent's history, including the call that spawned it."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "k1", "fork it", subagent_type="fork")
    k1 = proj.agent(SID, "k1", {"description": "fork it", "agentType": "fork", "toolUseId": call}, kind="meta_fork")
    k1.raw(m.lines[0])  # the copied history
    k1.raw(m.lines[0].replace('"agentId"', '"x"'))
    k1.prompt(BASE + 1, "brief")
    write(m, k1)
    s = load(proj)
    assert s.parent_of("k1") == MAIN


def test_workflow_agents_journal_phases_and_a_killed_run(tmp_path):
    proj, m = session(tmp_path)
    m.prompt(BASE, "run it")
    m.workflow(BASE + 1, "wf_1", "task1", "build-all")
    proj.journal(SID, "wf_1", [{"type": "launched"},
                               {"type": "started", "agentId": "w1", "label": "author:x", "phase": "Build"},
                               {"type": "result", "agentId": "w1", "result": {}},
                               {"type": "started", "agentId": "w2", "label": "review:x:r1", "phase": "Review"}])
    for aid, phase in (("w1", "Build"), ("w2", "Review"), ("w3", "Merge")):
        w = proj.agent(SID, aid, {"description": aid, "workflowPhase": phase}, run_id="wf_1", kind="meta_workflow")
        w.prompt(BASE + 2 * M, "brief")
        w.bash(BASE + 3 * M, "work", pending=(aid == "w2"))
        w.write()
    m.write()
    s = load(proj)
    now = BASE + 5 * M
    assert s.status_of("w1", now)[0] == "done"  # journal result
    assert s.status_of("w2", now)[0] == "run"
    assert s.run_live("wf_1") is True
    m.notify(BASE + 6 * M, "task1", "toolu_wf", "killed")
    m.write()
    s = load(proj)
    assert s.run_live("wf_1") is False
    assert s.status_of("w2", BASE + 7 * M)[0] == "cxl"  # unfinished when its run was killed
    assert s.status_of("w1", BASE + 7 * M)[0] == "done"


def test_the_fold_does_not_depend_on_record_order(tmp_path):
    proj, m = session(tmp_path)
    m.prompt(BASE, "go")
    call = m.launch(BASE + 1, "a1", "x")
    m.notify(BASE + 2 * M, "a1", call, "failed")
    m.resume(BASE + 3 * M, "a1")
    m.notify(BASE + 5 * M, "a1", call, "completed")
    call2 = m.launch(BASE + 6 * M, "a2", "y")
    m.notify(BASE + 7 * M, "a2", call2, "killed")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE + 1, "brief")
    a2 = agent(proj, "a2", toolUseId=call2)
    a2.prompt(BASE + 6 * M, "brief")
    write(m, a1, a2)
    base = load(proj)
    want = {a: base.status_of(a, BASE + 9 * M) for a in ("a1", "a2")}
    for seed in range(5):
        lines = list(m.lines)
        random.Random(seed).shuffle(lines)
        (proj.dir / f"{SID}.jsonl").write_text("\n".join(lines) + "\n")
        s = load(proj)
        assert {a: s.status_of(a, BASE + 9 * M) for a in ("a1", "a2")} == want
    assert want["a1"][0] == "done" and want["a2"][0] == "cxl"


def test_titles_and_prompts(tmp_path):
    proj, m = session(tmp_path)
    m.title("custom_title", "billing-api")
    m.title("agent_name", "billing")
    m.prompt(BASE, "first task")
    m.prompt(BASE + 5 * M, "legacy prompt", legacy=True)
    m.notify(BASE + 6 * M, "zz", "toolu_z", "completed", carriers=("user",))  # a notice is not a prompt
    m.write()
    s = load(proj)
    assert s.titles == {"custom": "billing-api", "agent": "billing"}
    assert [t for _, t in sorted(s.prompts)] == ["first task", "legacy prompt"]
    assert s.prompt_before(BASE + 6 * M) == (1, "legacy prompt")


def test_incremental_reads_partial_lines_and_rewrites(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    path = proj.dir / f"{SID}.jsonl"
    s = load(proj)
    assert s.status_of("a1", BASE + 2 * M)[0] == "run"
    m.notify(BASE + 3 * M, "a1", call, "completed")
    full = "\n".join(m.lines) + "\n"
    old = path.read_text()
    cut = len(old) + 40
    path.write_text(full[:cut])  # the notice is half written
    s.update()
    assert s.status_of("a1", BASE + 4 * M)[0] in ("run", "done")
    path.write_text(full)
    s.update()
    assert s.status_of("a1", BASE + 4 * M)[0] == "done"
    path.write_text(old)  # rewritten shorter: everything is rebuilt from the new content
    s.update()
    assert "a1" not in s.notices


def test_old_agent_files_are_skipped_until_they_change(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    os.utime(a1.path, (BASE, BASE))
    s = Session(SID, str(proj.dir / f"{SID}.jsonl"))
    s.update(since=BASE + 3600)
    assert s.file_of("a1").tail.offset == 0  # not read
    a1.bash(BASE + 2 * 3600, "again")
    a1.write()
    s.update(since=BASE + 3600)
    assert s.file_of("a1").tail.offset > 0


def test_ask_operator_receipts_belong_to_the_asking_agent(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.ask_operator(BASE + M, 7, "Which one?")
    write(m, a1)
    assert load(proj).qids == {"a1": {7}}


def test_meta_read_mid_write_is_retried(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    meta = proj.dir / SID / "subagents" / "agent-a1.meta.json"
    good = meta.read_text()
    meta.write_text(good[:10])
    s = load(proj)
    assert s.agents["a1"].meta == {}
    meta.write_text(good)
    s.update()
    assert json.loads(good)["description"] == s.agents["a1"].meta["description"]


# -- review of PR #1 -------------------------------------------------------------------

def test_h1_foreground_agent_runs_while_its_spawning_call_is_open(tmp_path):
    """The parent's open Agent call is ground truth: a long silent model turn is not an ending."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "f1", "plan", background=False)  # no tool_result yet
    f1 = agent(proj, "f1", toolUseId=call, requestShape="foreground")
    f1.prompt(BASE, "brief")
    f1.bash(BASE + 1 * M, "look")
    write(m, f1)
    s = load(proj)
    assert s.status_of("f1", BASE + 5 * M)[0] == "run"  # 4 min of thinking
    assert s.status_of("f1", BASE + M + PENDING_CAP + 60)[0] == "done"  # capped like abandoned calls


def test_m1_a_huge_last_line_keeps_the_timestamp_and_the_call_in_flight(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "f1", "plan", background=False)
    f1 = agent(proj, "f1", toolUseId=call, requestShape="foreground")
    f1.prompt(BASE, "brief")
    f1.bash(BASE + 1 * M, "x" * 100_000, pending=True)  # a >64 KB tool_use line, still running
    write(m, f1)
    s = load(proj)
    info = s.file_of("f1").info
    assert info.last_ts == BASE + 1 * M and len(info.pending) == 1
    assert s.status_of("f1", BASE + 5 * M)[0] == "run"


def test_m2_every_notice_block_in_a_carrier_counts(tmp_path):
    from .scenario import notification, tpl
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    rec = tpl("user_prompt")
    rec.pop("origin", None)
    rec["message"]["content"] = notification("a1", call, "failed") + "\n" + notification("bash9", "toolu_b", "completed")
    m.add(rec, BASE + 2 * M)
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    assert load(proj).status_of("a1", BASE + 3 * M)[0] == "fail"


def test_m3_journal_failed_is_a_failure(tmp_path):
    proj, m = session(tmp_path)
    m.workflow(BASE + 1, "wf_1", "task1", "build")
    proj.journal(SID, "wf_1", [{"type": "launched"},
                               {"type": "started", "agentId": "w1", "label": "author:x", "phase": "Build"},
                               {"type": "failed", "agentId": "w1"}])
    w = proj.agent(SID, "w1", {"description": "w1", "workflowPhase": "Build"}, run_id="wf_1", kind="meta_workflow")
    w.prompt(BASE + 2 * M, "brief")
    w.write()
    m.notify(BASE + 5 * M, "task1", "toolu_wf", "completed")
    m.write()
    assert load(proj).status_of("w1", BASE + 6 * M)[0] == "fail"


def test_l3_a_failed_resume_does_not_reopen(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    m.notify(BASE + 2 * M, "a1", call, "failed")
    m.resume(BASE + 3 * M, "a1")
    line = json.loads(m.lines[-1])
    line["toolUseResult"]["success"] = False
    m.lines[-1] = json.dumps(line)
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    assert load(proj).status_of("a1", BASE + 3.5 * M)[0] == "fail"


def test_l1_a_bad_record_does_not_break_the_session(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    m.raw(json.dumps({"type": "user", "timestamp": ["not", "a", "time"], "message": {"content": "x"}}))
    m.raw(json.dumps({"type": "user", "timestamp": {"a": 1}, "toolUseResult": {"resumedAgentId": ["x"]},
                      "message": {"content": [{"type": "tool_result", "tool_use_id": call}]}}))
    m.notify(BASE + 2 * M, "a1", call, "completed")
    a1 = agent(proj, "a1", toolUseId=call, spawnDepth="two")
    a1.prompt(BASE, "brief")
    write(m, a1)
    assert load(proj).status_of("a1", BASE + 3 * M)[0] == "done"


def test_l4_tool_calls_count_only_the_agents_own_calls(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + M, "read a transcript", output='{"type":"tool_use"} {"type":"tool_use"}')
    write(m, a1)
    assert load(proj).file_of("a1").tool_calls == 1


def test_h1_a_workflow_agent_runs_while_its_run_is_live(tmp_path):
    proj, m = session(tmp_path)
    m.workflow(BASE + 1, "wf_1", "task1", "build")
    proj.journal(SID, "wf_1", [{"type": "launched"},
                               {"type": "started", "agentId": "w1", "label": "author:x", "phase": "Build"}])
    w = proj.agent(SID, "w1", {"description": "w1", "workflowPhase": "Build"}, run_id="wf_1", kind="meta_workflow")
    w.prompt(BASE + 2 * M, "brief")
    w.bash(BASE + 3 * M, "work")
    write(m, w)
    s = load(proj)
    assert s.status_of("w1", BASE + 8 * M)[0] == "run"  # 5 min of silence inside a live run


def test_a_relaunched_run_abandons_the_old_instances_unfinished_agents(tmp_path):
    proj, m = session(tmp_path)
    m.workflow(BASE, "wf_1", "task1", "build")
    proj.journal(SID, "wf_1", [{"type": "launched"},
                               {"type": "started", "agentId": "old", "label": "author:x", "phase": "Build"},
                               {"type": "started", "agentId": "new", "label": "author:x", "phase": "Build"}])
    for aid, start in (("old", BASE + M), ("new", BASE + 11 * M)):
        w = proj.agent(SID, aid, {"description": aid, "workflowPhase": "Build"}, run_id="wf_1", kind="meta_workflow")
        w.prompt(start, "brief")
        w.write()
    m.workflow(BASE + 10 * M, "wf_1", "task2", "build")  # resumed: a new launch of the same run
    m.write()
    s = load(proj)
    assert s.status_of("old", BASE + 12 * M)[0] == "cxl"
    assert s.status_of("new", BASE + 14 * M)[0] == "run"


# -- re-review of PR #1 -----------------------------------------------------------------

def test_n1_a_fork_of_a_subagent_nests_under_it_even_when_only_its_own_copy_is_read(tmp_path):
    """The fork's own file repeats its spawning call; the real parent's file may be skipped (since)."""
    proj, m = session(tmp_path)
    call = m.launch(BASE, "p1", "parent")
    p1 = agent(proj, "p1", toolUseId=call)
    p1.prompt(BASE, "brief")
    inner = p1.launch(BASE + M, "k1", "fork it", subagent_type="fork")
    write(m, p1)
    k1 = proj.agent(SID, "k1", {"description": "fork it", "agentType": "fork", "toolUseId": inner,
                                "spawnDepth": 2, "parentAgentId": "p1"}, kind="meta_fork")
    k1.raw(p1.lines[1])  # the copied history: the spawning call
    k1.prompt(BASE + 2 * M, "brief")
    k1.write()
    os.utime(p1.path, (BASE, BASE))  # the parent's file is old: skipped by `since`
    s = Session(SID, str(proj.dir / f"{SID}.jsonl"))
    s.update(since=BASE + 60)
    assert s.file_of("p1").tail.offset == 0
    assert s.parent_of("k1") == "p1"


def test_n2_an_agent_waiting_on_a_live_child_is_running(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "p1", "parent")
    p1 = agent(proj, "p1", toolUseId=call)
    p1.prompt(BASE, "brief")
    inner = p1.launch(BASE + M, "c1", "child")
    p1.text(BASE + M + 5, "Started the child; waiting for it.")  # turn ended, no call in flight
    c1 = proj.agent(SID, "c1", {"description": "child", "toolUseId": inner, "spawnDepth": 2}, kind="meta_nested")
    c1.prompt(BASE + M, "brief")
    c1.bash(BASE + 29 * M, "long work")
    write(m, p1, c1)
    s = load(proj)
    assert s.status_of("c1", BASE + 30 * M)[0] == "run"
    assert s.status_of("p1", BASE + 30 * M)[0] == "run"  # silent for 29 min, but its child runs
    assert s.status_of("p1", BASE + 29 * M + ASYNC_QUIET + 60)[0] == "done"  # nothing runs any more


# -- the opt-in hook (hooks.py) ------------------------------------------------------

def hook(event, agent, when, pid=None, pstart=None, pidns="own"):
    from herdr_panorama import hooks
    path = hooks.hooks_path(SID)
    path.parent.mkdir(parents=True, exist_ok=True)
    rec = {"v": 1, "event": event, "agent": agent, "agent_type": "general-purpose", "ts": when}
    if pid is not None:
        rec.update(pid=pid, pstart=pstart, pidns=hooks.own_pidns() if pidns == "own" else pidns)
    with open(path, "a") as fh:
        fh.write(json.dumps(rec) + "\n")


def dead_process():
    import subprocess
    import sys
    from herdr_panorama import hooks
    p = subprocess.Popen([sys.executable, "-c", "pass"])
    start = hooks._stat(p.pid)[2]
    p.wait()
    return p.pid, start


def test_a_recorded_stop_is_a_confirmed_ending(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    hook("start", "a1", BASE + 1)
    hook("stop", "a1", BASE + 2 * M)
    assert load(proj).status_of("a1", BASE + 30 * M) == ("done", BASE + 2 * M, False)  # not inferred


def test_a_notice_about_the_same_stop_says_how_it_ended(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    m.notify(BASE + 2 * M + 1, "a1", call, "failed")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    hook("stop", "a1", BASE + 2 * M)
    assert load(proj).status_of("a1", BASE + 3 * M)[0] == "fail"


def test_a_recorded_start_reopens(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    m.notify(BASE + 2 * M, "a1", call, "completed")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    hook("start", "a1", BASE + 5 * M, os.getpid(), __import__("herdr_panorama.hooks").hooks._stat(os.getpid())[2])
    assert load(proj).status_of("a1", BASE + 5.5 * M)[0] == "run"


def test_started_never_stopped_and_process_gone_is_lost(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    a1.bash(BASE + M, "work")
    write(m, a1)
    pid, start = dead_process()
    hook("start", "a1", BASE + 1, pid, start)
    status, _, inferred = load(proj).status_of("a1", BASE + 2 * M)
    assert status == "lost" and not inferred
    # same agent, its Claude process still alive: not lost
    from herdr_panorama import hooks
    hooks.hooks_path(SID).unlink()
    hook("start", "a1", BASE + 1, os.getpid(), hooks._stat(os.getpid())[2])
    assert load(proj).status_of("a1", BASE + 2 * M)[0] == "run"
    # without process info nothing is called lost
    hooks.hooks_path(SID).unlink()
    hook("start", "a1", BASE + 1)
    assert load(proj).status_of("a1", BASE + 2 * M + ASYNC_QUIET)[0] == "done"



def test_m2_no_lost_across_a_session_resume(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    pid, start = dead_process()
    hook("start", "a1", BASE + 1, pid, start)
    from herdr_panorama import hooks
    hook("session", "", BASE + 5 * M, os.getpid(), hooks._stat(os.getpid())[2])  # resumed in a new process
    status, _, inferred = load(proj).status_of("a1", BASE + 30 * M)
    assert status == "done" and inferred  # done?, not lost


def test_m2_no_lost_from_another_pid_namespace(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    pid, start = dead_process()
    hook("start", "a1", BASE + 1, pid, start, pidns="pid:[4026531999]")
    assert load(proj).status_of("a1", BASE + 30 * M)[0] == "done"


def test_l2_hook_data_survives_a_rebuild(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    hook("stop", "a1", BASE + 2 * M)
    s = load(proj)
    path = proj.dir / f"{SID}.jsonl"
    path.write_text(path.read_text()[: len(path.read_text()) // 2].rsplit("\n", 1)[0] + "\n")  # rewritten shorter
    m.write()  # and back: a RESET and rebuild
    s.update()
    assert s.hook_stops.get("a1") and s.status_of("a1", BASE + 3 * M) == ("done", BASE + 2 * M, False)


def test_l3_a_late_failed_notice_still_wins(tmp_path):
    proj, m = session(tmp_path)
    call = m.launch(BASE, "a1", "x")
    m.notify(BASE + 5 * M, "a1", call, "failed")  # 3 min after the recorded stop
    a1 = agent(proj, "a1", toolUseId=call)
    a1.prompt(BASE, "brief")
    write(m, a1)
    hook("stop", "a1", BASE + 2 * M)
    assert load(proj).status_of("a1", BASE + 6 * M)[0] == "fail"
