"""Wider cards on big screens (design turn 7, approved by the operator): card widths by a tier
of the terminal width, a sweep lap of at most 12 s whatever the width, the model's version.
Golden tests against docs/design/swarm-wide.js rendered by render-7.mjs
(tests/fixtures/design_7.json), cell for cell where the design draws the turn-5 depth columns."""

import pytest

from herdr_panorama import render
from herdr_panorama.collect import short_model

from .design_wide import NOW, TID, WIDE, wide_swarm

FRAMES = WIDE["frames"]


def norm(ch, fg, bg, bold):
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def r1_id():
    t = wide_swarm().orchestrators[0].threads[0]
    return next(a.id for a in t.agents if a.depth == 2 and a.activity.startswith("tracing"))


@pytest.mark.parametrize("frame", sorted(FRAMES))
@pytest.mark.parametrize("key,w,focus,x0", [("W160", 160, "thread", 89), ("W240", 240, "thread", 99),
                                             ("W320", 320, "thread", 111), ("R160", 160, "r1", 0),
                                             ("R320", 320, "r1", 0)])
def test_wide_cards_match_the_design_cell_for_cell(frame, key, w, focus, x0):
    """W*: focus on the thread (from the thread→agent links rightwards: the orchestrator and
    thread columns are the design's mock); R*: focus on depth 2, the whole width. Rows 2..41."""
    sel = None if focus == "thread" else r1_id()
    v = render.View(orch="w1:p3", thread=TID, agent=sel, now=NOW, frame=int(frame), focus=1 if sel is None else 3)
    scr = render.render(wide_swarm(), v, w, 45)
    diffs = []
    for y in range(2, 42):
        want = [norm(ch, r[1], r[2], r[3]) for r in FRAMES[frame][key][y] for ch in r[0]]
        got = [norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y]]
        diffs += [(x, y, want[x], got[x]) for x in range(x0, w) if want[x] != got[x]]
    assert not diffs, f"{len(diffs)} cells differ, first: {diffs[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES))
def test_the_sweep_strip_matches_the_design(frame):
    """28, 44, 56 and 60-column working cards side by side, as on the design page."""
    scr = render.Screen(206, 4)
    x = 1
    working = render.OrchState(render.RUN, "⠋", "working", True)
    for w in (28, 44, 56, 60):
        render.rbox(scr, x, 0, w, 3, render.LINE)
        render.edge(scr, x, 0, w, 3, working, int(frame), 0)
        x += w + 3
    want = "".join(ch for r in FRAMES[frame]["S"][2] for ch in r[0])
    assert "".join(c.ch for c in scr.c[2]) == want


def test_a_lap_is_at_most_12_seconds_and_narrow_cards_are_unchanged():
    assert render.SWEEP_LAP == 48  # 250 ms frames

    def bar_start(w, frame):
        scr = render.Screen(w, 3)
        render.edge(scr, 0, 0, w, 3, render.OrchState(render.RUN, "⠋", "working", True), frame, 0)
        row = "".join(c.ch for c in scr.c[2])
        return row.find("━")

    for w in (44, 56, 60, 64):
        assert [bar_start(w, f) for f in range(48)] == [bar_start(w, f + 48) for f in range(48)]  # one lap: 48
    assert [bar_start(28, f) for f in range(32)] == [bar_start(28, f + 32) for f in range(32)]  # as before


@pytest.mark.parametrize("w,expect", [(120, (28, 36, 44)), (160, (28, 50, 44)), (219, (28, 50, 44)),
                                      (220, (32, 56, 56)), (299, (32, 56, 56)), (300, (36, 64, 60)),
                                      (500, (36, 64, 60))])
def test_tiers(w, expect):
    assert render.tier(w) == expect


def test_the_full_title_fits_from_240_columns():
    sel = r1_id()
    for w, whole in ((160, False), (240, True), (320, True)):
        v = render.View(orch="w1:p3", thread=TID, agent=sel, now=NOW, focus=3)
        text = render.render(wide_swarm(), v, w, 45).text()
        assert (f"general-purpose {sel} · opus 4.5" in text) is whole


@pytest.mark.parametrize("raw,short", [("claude-opus-4-5-20251101", "opus 4.5"), ("claude-sonnet-4-5", "sonnet 4.5"),
                                       ("claude-3-5-sonnet-20241022", "sonnet 3.5"), ("claude-sonnet-4-20250514", "sonnet 4"),
                                       ("claude-haiku-4-5-20251001", "haiku 4.5"), ("opus", "opus"), (None, None),
                                       ("some-other-model", "some-other-model")])
def test_the_model_keeps_its_version(raw, short):
    assert short_model(raw) == short
