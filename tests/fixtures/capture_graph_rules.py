"""Capture tests/fixtures/graph_rules.json: Python's wide-graph columns (render.orch_column,
render.thread_column, render.agent_card) on a small swarm built to exercise their rules
(questions no agent filed, active/live counts, a long name, many pods and nested ones, a
thread never started, the folded done threads unfolded, a waiting agent with two background
tasks and a phase index from the end, a card on the selected path). The Rust port compares its
own columns with it (crates/panorama-core/tests/graph_columns.rs).

    uv run python tests/fixtures/capture_graph_rules.py > tests/fixtures/graph_rules.json
"""

import json
from herdr_panorama import render as r, protocol
from herdr_panorama.model import Swarm, Orchestrator, Thread, Agent, Phase, BgTask
NOW=1_800_000_000.0
ags=[Agent(id=f"a{i}", status="run", started=NOW-600+i, seq=i) for i in range(9)]
ags.append(Agent(id="main:w1:p1", status="run", started=NOW-900))
ags.append(Agent(id="n1", status="bg", started=NOW-3000, phases=[-1],
                 bg=[BgTask("b1","Bash","x",NOW-600), BgTask("b2","Bash","y",NOW-1800)]))
ags.append(Agent(id="k1", parent="a0", status="run", started=NOW-100))
t1=Thread(id="t1", name="Thread one", source="explicit", status="active", agents=ags,
          phases=[Phase("plan","done"),Phase("build","now")])
t2=Thread(id="main:w1:p1", name="session", source="session", status="active")
t3=Thread(id="t3", name="Done one", source="turn", status="done")
o=Orchestrator(pane_id="w1:p1", tab_id="w1:t1", tab_label="A very long orchestrator tab label here", session_id=None,
               herdr_status="working", tab_no=1, threads=[t1,t2,t3])
o.inbox=[7,8]
o2=Orchestrator(pane_id="w1:p2", tab_id="w1:t2", tab_label="Second", session_id=None,
               herdr_status="working", tab_no=2, threads=[t1,t2,t3])
sw=Swarm(orchestrators=[o,o2], generated_at=NOW, herdr_ok=True, inbox_ok=True)
v=r.View(orch="w1:p1", thread="t1", agent="n1", focus=2, now=NOW, done_open={"w1:p1"})
oc,_=r.orch_column(sw, v, 28); tc,_=r.thread_column(o, v, 50)
cv=r.Screen(44,20); h=r.agent_card(cv,0,0,44,ags[10],t1,v,True,2)
cv2=r.Screen(44,3); r.agent_card(cv2,0,0,44,ags[0],t1,v,True,2,on_path=True)
def txt(s,n): return "\n".join(s.text().split("\n")[:n])
print(json.dumps({"source": "tests/fixtures/capture_graph_rules.py", "now": NOW, "swarm": protocol.swarm_to_wire(sw), "orchs": txt(oc.canvas,13), "threads": txt(tc.canvas,20), "n1": txt(cv,h), "n1h": h,
  "a0_fg": str(cv2.c[1][6].fg), "a0_border": str(cv2.c[0][3].fg)}, ensure_ascii=False, indent=1))
