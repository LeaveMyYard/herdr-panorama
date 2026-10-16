"""The design's sample dataset (captured from docs/design/swarm-tui.js into
tests/fixtures/design_data.json) as a model Swarm, for the 3a dock panel golden test and its
screenshots. Panorama is docked next to tab 3 ("herdr core"), as in the design."""

from __future__ import annotations

import json
from pathlib import Path

from herdr_panorama.model import BLK, Agent, Orchestrator, Phase, Swarm, Thread

FIX = Path(__file__).resolve().parent / "fixtures"
PH = {"d": "done", "n": "now", "x": "next", "f": "failed"}


def design_swarm(data: list | None = None) -> Swarm:
    """The dataset as a Swarm; ``data`` another capture of it (design 9 adds an orchestrator)."""
    data = data if data is not None else json.loads((FIX / "design_data.json").read_text())
    sw = Swarm(herdr_ok=True, inbox_ok=True)
    qid = 0
    for o in data:
        orch = Orchestrator(pane_id=f"w1:p{o['tab']}", tab_id=f"w1:t{o['tab']}", tab_label=o["name"], session_id=None,
                            herdr_status="idle", tab_no=o["tab"])
        for t in o["threads"]:
            th = Thread(id=t["id"], name=t["name"], source="explicit", phases=[Phase(n, PH[s]) for n, s in t["ph"]])
            th.status = "done" if all(s == "d" for _, s in t["ph"]) else "active"
            for a in t["agents"]:
                ids = list(range(qid + 1, qid + 1 + a["q"]))
                qid += a["q"]
                th.agents.append(Agent(id=a["id"], role=a["role"], model=a["model"], status=a["st"], seq=a["n"],
                                       question_ids=ids, activity=a.get("act") or "",
                                       last_message=a.get("msg") or "", questions=list(a.get("qs") or []),
                                       error=a.get("err"),
                                       pending=[f"wants {a['perm']}"] if a.get("perm") else []))
                orch.inbox += ids
            orch.threads.append(th)
        if any(a.status == BLK for a in orch.agents()):
            orch.herdr_status = "blocked"  # a subagent's permission prompt blocks the session's pane
        sw.orchestrators.append(orch)
    sw.docked = "w1:p3"
    return sw
