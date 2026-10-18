"""Record Python's unseen tracker (unseen.Tracker) over a fixed sequence of refreshes, for the
Rust one: after each step, the marks and the state file's exact bytes.

    uv run python tools/capture_unseen.py > tests/fixtures/unseen/steps.json

Each step is a swarm (orchestrators with their agents), a time, whether the tracker restarts
first (a new process on the same file), and agents marked seen after the update.
"""
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))
from herdr_panorama import unseen  # noqa: E402
from herdr_panorama.model import Agent, Orchestrator, Swarm, Thread  # noqa: E402

DAY = 86400.0


def o(pane, agents, focused=False, machine=None, machine_id=None):
    return {"pane": pane, "focused": focused, "machine": machine, "machine_id": machine_id, "agents": agents}


def a(aid, status, qids=(), ended=None, last_at=None, started=None):
    return {"id": aid, "status": status, "qids": list(qids), "ended": ended, "last_at": last_at, "started": started}


STEPS = [
    {"now": 100.0, "swarm": [o("w1:p1", [a("a", "run"), a("b", "run"), a("c", "cxl")]),
                             o("m1:w1:p1", [a("x", "run")], machine="mä", machine_id="mä")]},
    {"now": 101.0, "swarm": [o("w1:p1", [a("a", "fail"), a("b", "wait", [7]), a("c", "done")]),
                             o("m1:w1:p1", [a("x", "inf")], machine="mä", machine_id="mä")]},
    {"now": 102.0, "restart": True,
     "swarm": [o("w1:p1", [a("a", "fail"), a("b", "wait", []), a("c", "done"),
                           a("eq", "done", ended=101.0), a("old", "done", ended=50.0), a("new", "fail", last_at=150.0)]),
               o("m1:w1:p1", [a("x", "inf")], machine="mä", machine_id="mä")]},
    {"now": 103.0, "swarm": [o("w1:p1", [a("a", "fail"), a("b", "run"), a("c", "done"), a("eq", "done")])],
     "seen": ["a"]},
    {"now": 104.0, "swarm": [o("w1:p1", [a("a", "fail"), a("b", "done", [9])], focused=True)]},
    {"now": 105.0 + 8 * DAY, "restart": True, "swarm": [o("w1:p1", [a("b", "done")])]},
]


def swarm(spec):
    orchs = []
    for x in spec:
        ags = [Agent(id=g["id"], status=g["status"], question_ids=g["qids"], ended=g["ended"], last_at=g["last_at"],
                     started=g["started"]) for g in x["agents"]]
        oo = Orchestrator(pane_id=x["pane"], tab_id="t", tab_label="o", session_id="s", focused=x["focused"],
                          threads=[Thread(id="t1", name="t", source="turn", agents=ags)])
        oo.machine, oo.machine_id = x["machine"], x["machine_id"]
        orchs.append(oo)
    return Swarm(orchestrators=orchs)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "unseen.json"
        tracker = None
        out = []
        for step in STEPS:
            if tracker is None or step.get("restart"):
                tracker = unseen.Tracker(path=path, enabled_fn=lambda: True)
            marks = tracker.update(swarm(step["swarm"]), step["now"])
            if step.get("seen"):
                tracker.save_seen(tracker.seen(step["seen"]))
            out.append({**step, "marks": marks, "file": path.read_text()})
    doc = {"source": "tools/capture_unseen.py: herdr_panorama.unseen.Tracker", "steps": out}
    print(json.dumps(doc, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
