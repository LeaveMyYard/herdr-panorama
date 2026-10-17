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


def dist(a: str, b: str) -> float:
    return sum((int(a[i:i + 2], 16) - int(b[i:i + 2], 16)) ** 2 for i in (1, 3, 5)) ** 0.5


FRAMES = [f / 10 for f in range(0, 121)]  # a whole pulse (12 frames) at 0.1-frame steps, peak included


@pytest.mark.parametrize("event", ["fail", "wait", "done"])
@pytest.mark.parametrize("focused", [True, False])
def test_the_selected_card_stays_apart_through_a_whole_pulse(event, focused):
    """Review of #46: a failed or question pulse reaches the full event colour at its peak; the
    selected card must still differ there. Focused (white selection): brighter by a margin at
    every point; selected in another column (grey): a clearly different colour."""
    sw = demo_swarm()
    a, b = sw.orchestrators[0], sw.orchestrators[1]
    unseen = {a.agents()[0].id: event, b.agents()[0].id: event}  # both pulse, the same event
    worst_lum, worst_dist = 1e9, 1e9
    for frame in FRAMES:
        v = render.View(orch=a.pane_id, focus=0 if focused else 1, frame=frame, unseen=unseen, now=NOW)
        scr = render.render(sw, v, 160, 45)
        sel, other = top_border(scr, a.tab_label), top_border(scr, b.tab_label)
        worst_lum = min(worst_lum, lum(sel) - lum(other))
        worst_dist = min(worst_dist, dist(sel, other))
    if focused:
        assert worst_lum >= 15 and worst_dist >= 50, (event, worst_lum, worst_dist)
    else:
        assert worst_dist >= 40, (event, worst_dist)


def test_without_a_pulse_the_selection_is_unchanged():
    sw = demo_swarm()
    a = sw.orchestrators[0]
    v = render.View(orch=a.pane_id, focus=0, frame=0, now=NOW)
    assert top_border(render.render(sw, v, 160, 45), a.tab_label) == render.HI
