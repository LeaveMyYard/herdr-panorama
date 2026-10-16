"""No two status glyphs in adjacent cells, anywhere, and no half-circle spinner at all: the
operator's ghostty drew ◐◑◒◓ from a fallback font wider than one cell, so they overlapped their
neighbours. The spinner is braille now (drawn cell-exact), and glyph rows keep a space between
glyphs."""

import pytest

from herdr_panorama import render

from .demo import NOW, demo_swarm
from .design_deep import NOW as DNOW, TID, deep_swarm
from .design_dock import design_swarm

GLYPHS = set("⠋⠼⠧⠹○✓✗")


def adjacent(scr):
    out = []
    for y in range(scr.h):
        row = [c.ch for c in scr.c[y]]
        out += [(x, y, row[x] + row[x + 1]) for x in range(len(row) - 1) if row[x] in GLYPHS and row[x + 1] in GLYPHS]
    return out


def screens():
    for frame in range(4):
        for w in (46, 60):
            sw = design_swarm()
            sw.docked = "w1:p3"
            yield f"dock {w} f{frame}", render.render(sw, render.View(orch="w1:p3", dock=True, frame=frame), w, 45)
        sw = deep_swarm()
        sw.docked = "w1:p3"
        yield f"deep dock f{frame}", render.render(sw, render.View(orch="w1:p3", dock=True, frame=frame, now=DNOW), 46, 45)
        for w in (70, 120, 160, 240):
            yield f"demo {w} f{frame}", render.render(
                demo_swarm(), render.View(orch="w1:p1", thread="ref:#42", agent="a2f81c0", focus=2, now=NOW, frame=frame), w, 45)
            yield f"deep {w} f{frame}", render.render(
                deep_swarm(), render.View(orch="w1:p3", thread=TID, agent="c1", focus=5, now=DNOW, frame=frame), w, 45)


@pytest.mark.parametrize("name,scr", list(screens()), ids=lambda v: v if isinstance(v, str) else "")
def test_no_two_glyphs_touch(name, scr):
    assert adjacent(scr) == []


def test_the_docked_rows_space_their_glyphs():
    sw = design_swarm()
    sw.docked = "w1:p3"
    text = render.render(sw, render.View(orch="w1:p3", dock=True), 46, 45).text()
    row = next(r for r in text.split("\n") if "3 active · 5 agents" in r)
    glyphs = row.split("  3 active")[0].strip(" ▌")
    assert glyphs == "? ⠋ ⠧ ✓"  # was "?⠋⠧✓"


def many_threads(n):
    from herdr_panorama.model import RUN, Agent, Orchestrator, Swarm, Thread
    o = Orchestrator(pane_id="w1:p1", tab_id="t", tab_label="busy", session_id="s", tab_no=1,
                     threads=[Thread(id=f"t{i}", name=f"thread {i}", source="turn",
                                     agents=[Agent(id=f"a{i}", status=RUN, seq=i)]) for i in range(n)])
    return Swarm(orchestrators=[o])


@pytest.mark.parametrize("n", [15, 30])
@pytest.mark.parametrize("w", [36, 46])
def test_review_m_many_threads_keep_the_counts_whole(n, w):
    """Review #27: the spaced thread-glyph row overflowed at ~15 threads. The first glyphs then
    "+n" for the rest; the counts always fit, whole."""
    scr = render.render(many_threads(n), render.View(dock=True, orch="w1:p1"), w, 45)
    row = next(r for r in scr.text().split("\n") if "active · " in r)
    assert row.rstrip().endswith(f"{n} active · {n} agents") and len(row.rstrip()) <= w
    shown = row.strip(" ▌").split("  ")[0].split()
    assert shown[-1].startswith("+") and len(shown) - 1 + int(shown[-1][1:]) == n
    assert adjacent(scr) == []


def test_a_few_threads_show_every_glyph():
    scr = render.render(many_threads(3), render.View(dock=True, orch="w1:p1"), 46, 45)
    row = next(r for r in scr.text().split("\n") if "active · " in r)
    assert "+" not in row and len(row.strip(" ▌").split("  ")[0].split()) == 3


@pytest.mark.parametrize("name,scr", list(screens()), ids=lambda v: v if isinstance(v, str) else "")
def test_no_half_circle_glyphs_anywhere(name, scr):
    assert not any(c.ch in "◐◑◒◓" for row in scr.c for c in row)


def test_the_spinner_is_a_braille_quarter_turn():
    from herdr_panorama.model import GLYPH, RUN, SPIN
    assert SPIN == "⠋⠹⠼⠧" and GLYPH[RUN] == SPIN[0]
    assert all(0x2800 <= ord(ch) <= 0x28FF for ch in SPIN)
