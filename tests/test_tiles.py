"""The docked panel matches design 9e (turn 9, "tiles, list replaces row") cell for cell: 2a's
coloured tiles for each entry's state, one tile per thread on line 2 (finished threads fold into
one ✓n tile), and a tile per thread in the docked entry's list, whose line 2 then keeps only the
counts. Also with turn 6's unseen marks, where an unseen thread's tile pulses its background in
the event colour (the operator, Q115). Golden: docs/design/swarm-tiles.js rendered by
render-9.mjs (tests/fixtures/design_9.json)."""

import json

import pytest

from herdr_panorama import render

from .design_dock import FIX, design_swarm

NINE = json.loads((FIX / "design_9.json").read_text())
FRAMES = NINE["frames"]
MARKS = {"b3": "fail", "c1": "wait", "a5": "done", "i3": "wait", "k2": "done"}  # turn 6's sample


def norm(ch, fg, bg, bold):
    """A blank shows only its background: its colour and weight are invisible."""
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def grid(runs):
    return [[norm(ch, r[1], r[2], r[3]) for r in row for ch in r[0]] for row in runs]


def panel(frame, marks=None):
    v = render.View(orch="w1:p3", dock=True, frame=frame, unseen=dict(marks or {}))
    v.hover = ("orch", ("w1:p4",))  # the design's hover: Billing Migrations
    scr = render.render(design_swarm(NINE["data"]), v, 46, 45)
    return [[norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y][:46]] for y in range(45)]


def diff(want, got):
    return [(x, y, want[y][x], got[y][x]) for y in range(45) for x in range(46) if want[y][x] != got[y][x]]


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_9e_matches_the_design_cell_for_cell(frame):
    d = diff(grid(FRAMES[frame]["E"]), panel(int(frame)))
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_9e_unseen_tiles_pulse_exactly_like_the_design(frame):
    d = diff(grid(FRAMES[frame]["U"]), panel(int(frame), MARKS))
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


def test_an_unseen_thread_tile_pulses_its_background_never_fully_off():
    rows = [panel(f, MARKS) for f in range(12)]
    y = next(y for y in range(45) if "".join(c[0] for c in rows[0][y][9:21]) == "Bug: tab pre")
    bgs = {r[y][6][2] for r in rows}  # the tile's glyph cell
    assert len(bgs) > 3  # it moves through the 3 s pulse
    assert render.PB[render.WAIT][0] not in {r[y][6][2] for r in rows[5:8]}  # at its peak it is the event colour


def test_the_docked_line_two_keeps_only_the_counts():
    text = "".join(c[0] for c in panel(4)[9])
    assert text.strip(" ▌") == "3 active · 5 agents"
