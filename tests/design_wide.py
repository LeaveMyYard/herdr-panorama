"""Turn 7's dataset (captured from docs/design/swarm-wide.js into tests/fixtures/design_7.json)
as a model Swarm: turn 5's thread with real agent types, 7-hex ids and versioned models, for the
wide-cards golden test and its screenshots."""

from __future__ import annotations

import json
from pathlib import Path

from herdr_panorama.model import Agent, Orchestrator, Phase, Swarm, Thread

FIX = Path(__file__).resolve().parent / "fixtures"
WIDE = json.loads((FIX / "design_7.json").read_text())
PH = {"d": "done", "n": "now", "x": "next", "f": "failed"}
NOW = 1_800_000_000.0
TID = "x:deep"


def _minutes(age: str) -> int:
    return int(age.rstrip("m")) if age.endswith("m") else 0


def wide_swarm() -> Swarm:
    th = Thread(id=TID, name=WIDE["thread"]["name"], source="explicit", status="active", started=NOW - 44 * 60,
                phases=[Phase(n, PH[s]) for n, s in WIDE["thread"]["ph"]])
    qid = [0]

    def add(a: dict, parent: str | None, depth: int) -> None:
        ids = list(range(qid[0] + 1, qid[0] + 1 + a["q"]))
        qid[0] += a["q"]
        th.agents.append(Agent(id=a["id"], role=a["role"], model=a["model"], status=a["st"], seq=a["n"],
                               started=NOW - _minutes(a["age"]) * 60, tool_calls=a["tools"], activity=a["act"],
                               questions=list(a["qs"]), question_ids=ids, error=a["err"], parent=parent,
                               depth=depth, phases=[1]))
        for k in a["kids"]:
            add(k, a["id"], depth + 1)

    for a in WIDE["tree"]:
        add(a, None, 1)
    orch = Orchestrator(pane_id="w1:p3", tab_id="w1:t3", tab_label="Core Library", session_id=None,
                        herdr_status="working", tab_no=3, threads=[th])
    orch.inbox = [i for a in th.agents for i in a.question_ids]
    return Swarm(orchestrators=[orch], generated_at=NOW, herdr_ok=True, inbox_ok=True)
