"""Capture tests/fixtures/graph_rules.json: Python's wide-graph columns (render.orch_column,
render.thread_column, render.agent_card) on a small swarm built to exercise their rules
(questions no agent filed, active/live counts, a long name, many pods and nested ones, a
thread never started, the folded done threads unfolded with an unseen finished agent, the
orchestrator's background card with no turn end, a waiting agent with two background tasks
and a phase index from the end, a card on the selected path). Each canvas is stored as runs
of ``[text, fg, bg, bold]`` per row, the shape of ``screens/*.json`` (``null``: no
background). The Rust port compares its own columns with it, cell for cell
(crates/panorama-core/tests/graph_columns.rs).

    uv run python tests/fixtures/capture_graph_rules.py > tests/fixtures/graph_rules.json
"""

import json

from herdr_panorama import protocol
from herdr_panorama import render as r
from herdr_panorama.model import Agent, BgTask, Orchestrator, Phase, Swarm, Thread

NOW = 1_800_000_000.0


def runs(scr: r.Screen, rows: int) -> list[list]:
    """The first ``rows`` rows of ``scr`` as runs of ``[text, fg, bg, bold]``."""
    out = []
    for y in range(rows):
        row: list[list] = []
        for c in scr.c[y]:
            attrs = [str(c.fg), str(c.bg) if c.bg else None, bool(c.bold)]
            if row and row[-1][1:] == attrs:
                row[-1][0] += str(c.ch)
            else:
                row.append([str(c.ch), *attrs])
        out.append(row)
    return out


ags = [Agent(id=f"a{i}", status="run", started=NOW - 600 + i, seq=i) for i in range(9)]
ags.append(Agent(id="main:w1:p1", status="run", started=NOW - 900))
ags.append(Agent(id="n1", status="bg", started=NOW - 3000, phases=[-1],
                 bg=[BgTask("b1", "Bash", "x", NOW - 600), BgTask("b2", "Bash", "y", NOW - 1800)]))
ags.append(Agent(id="k1", parent="a0", status="run", started=NOW - 100))
t1 = Thread(id="t1", name="Thread one", source="explicit", status="active", agents=ags,
            phases=[Phase("plan", "done"), Phase("build", "now")])
t2 = Thread(id="main:w1:p1", name="session", source="session", status="active")
t3 = Thread(id="t3", name="Done one", source="turn", status="done",
            agents=[Agent(id="z1", status="done", started=NOW - 4000)])
o = Orchestrator(pane_id="w1:p1", tab_id="w1:t1", tab_label="A very long orchestrator tab label here",
                 session_id=None, herdr_status="working", tab_no=1, threads=[t1, t2, t3])
o.inbox = [7, 8]
o.bg = [BgTask("ob", "Monitor", "watch", NOW - 120)]
o.last_activity = 0.0  # no turn end: no "turn ended" tag
o2 = Orchestrator(pane_id="w1:p2", tab_id="w1:t2", tab_label="Second", session_id=None,
                  herdr_status="working", tab_no=2, threads=[t1, t2, t3])
sw = Swarm(orchestrators=[o, o2], generated_at=NOW, herdr_ok=True, inbox_ok=True)
v = r.View(orch="w1:p1", thread="t1", agent="n1", focus=2, now=NOW, done_open={"w1:p1"},
           unseen={"z1": "done"})
oc, oh = r.orch_column(sw, v, 28)
tc, th = r.thread_column(o, v, 50)
n1 = r.Screen(44, 20)
n1h = r.agent_card(n1, 0, 0, 44, ags[10], t1, v, True, 2)
a0 = r.Screen(44, 3)
r.agent_card(a0, 0, 0, 44, ags[0], t1, v, True, 2, on_path=True)
print(json.dumps({"source": "tests/fixtures/capture_graph_rules.py", "now": NOW, "unseen": v.unseen,
                  "swarm": protocol.swarm_to_wire(sw),
                  "orchs": runs(oc.canvas, oh), "threads": runs(tc.canvas, th),
                  "n1": runs(n1, n1h), "a0": runs(a0, 3)}, ensure_ascii=False, indent=1))
