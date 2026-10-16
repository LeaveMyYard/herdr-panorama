"""An agent card's title (role, id, model) never runs into its right-aligned state label or
"› n", at any card width: it is cut with "…" (the operator saw `· opurunning`)."""

import pytest

from herdr_panorama import render
from herdr_panorama.model import DONE, RUN, Agent, Orchestrator, Swarm, Thread


def swarm_with(agent):
    t = Thread(id="t", name="work", source="turn", agents=[agent, Agent(id="b2c3d4e", role="other", status=RUN)])
    o = Orchestrator(pane_id="w1:p1", tab_id="t1", tab_label="orch", session_id="s", threads=[t])
    return Swarm(orchestrators=[o])


@pytest.mark.parametrize("w", [120, 140, 160, 200, 320])
@pytest.mark.parametrize("status", [RUN, DONE])
def test_the_selected_title_stops_before_the_state(w, status):
    a = Agent(id="a6c8a7b1", role="general-purpose", model="opus 4.5", status=status, started=0.0,
              ended=60.0 if status == DONE else None)
    v = render.View(orch="w1:p1", thread="t", agent=a.id, focus=2, now=3600.0, fin_open={"t"})
    rows = render.render(swarm_with(a), v, w, 45).text().split("\n")
    row = next(r for r in rows if "general-pur" in r and "finished" not in r)
    label = render.LABEL[status]
    assert label in row
    title_end = row.index(label)
    assert row[title_end - 1] == " "  # at least one space between the title and the state
    assert "opurunning" not in row and "opudone" not in row
    head = row[: title_end].rstrip()
    assert head.endswith("opus 4.5") or head.endswith("…")


@pytest.mark.parametrize("w", [120, 160])
def test_an_unselected_title_is_cut_too(w):
    a = Agent(id="a6c8a7b1", role="general-purpose-very-long-agent-type", model="sonnet 4.5", status=RUN)
    v = render.View(orch="w1:p1", thread="t", agent="b2c3d4e", focus=2, now=3600.0)
    rows = render.render(swarm_with(a), v, w, 45).text().split("\n")
    row = next(r for r in rows if "general-purpose" in r)
    card = row[row.index("general-purpose") - 6:]
    border = card.index("│", 1) if "│" in card[1:] else len(card)
    assert "…" in card[:border]


def test_fit_segs_keeps_short_titles_and_cuts_long_ones():
    segs = [("general-purpose", "a"), (" a6c8a7b", "b"), (" · opus", "c")]
    assert render.fit_segs(segs, 40) == segs
    cut = render.fit_segs(segs, 20)
    assert "".join(g[0] for g in cut) == "general-purpose a6c…" and [g[1] for g in cut] == ["a", "b"]
