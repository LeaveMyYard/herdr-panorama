"""The operator's report: in the graph view the current (selected) card could not be told
apart while its border was animated: the unseen pulse recoloured the whole border over the
plain line colour, the selection's white included. The pulse now runs over the card's own
border colour, so the selected card stays the brightest at every point of the pulse."""

import pytest

from herdr_panorama import render

from .demo import NOW, demo_swarm


def lum(hexcol: str) -> float:
    r, g, b = (int(hexcol[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def top_border(scr, label: str) -> str:
    rows = scr.text().splitlines()
    y = next(i for i, r in enumerate(rows) if label in r[:60]) - 1  # the card's top border row
    x = next(i for i, ch in enumerate(rows[y]) if ch == "╭")
    return str(scr.c[y][x + 5].fg)


@pytest.mark.parametrize("frame", range(12))  # one whole pulse
def test_the_selected_card_stays_brightest_while_it_pulses(frame):
    sw = demo_swarm()
    a, b = sw.orchestrators[0], sw.orchestrators[1]
    unseen = {a.agents()[0].id: "done", b.agents()[0].id: "done"}  # both pulse, the same event
    v = render.View(orch=a.pane_id, focus=0, frame=frame, unseen=unseen, now=NOW)
    scr = render.render(sw, v, 160, 45)
    sel, other = top_border(scr, a.tab_label), top_border(scr, b.tab_label)
    assert sel != other and lum(sel) > lum(other) + 40, (frame, sel, other)


def test_without_a_pulse_the_selection_is_unchanged():
    sw = demo_swarm()
    a = sw.orchestrators[0]
    v = render.View(orch=a.pane_id, focus=0, frame=0, now=NOW)
    assert top_border(render.render(sw, v, 160, 45), a.tab_label) == render.HI
