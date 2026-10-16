"""Smooth colour animation (design: smooth animation): pulses and blinks are continuous in time, the colour
tick restyles only the animated cells between whole frames, and whole frames stay exactly the
stepped design frames (the goldens in test_unseen / test_dock_panel / test_deep hold them)."""

import itertools
import json
import math

import pytest

from herdr_panorama import render, tui
from herdr_panorama.model import FAIL
from herdr_panorama.render import FAINT, ST

from .demo import demo_swarm
from .design_deep import deep_swarm
from .design_dock import FIX, design_swarm
from .test_tui import loaded, make_app
from .test_unseen import MARKS, cells, grid

FRACTIONS = [0.3, 1.5, 2.7, 3.95, 5.25, 9.9, 11.6]


def rgb(c: str) -> tuple[int, int, int]:
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def dump(scr: render.Screen):
    return [[(c.ch, str(c.fg), str(c.bg) if c.bg else None, c.bold) for c in scr.c[y]] for y in range(scr.h)]


def layouts():
    """(name, swarm factory, view kwargs, width): every layout that animates colours."""
    return [
        ("2a unseen", design_swarm, {"orch": "w1:p1", "thread": "t595", "agent": "a2", "focus": 2, "unseen": dict(MARKS)}, 160),
        ("3a unseen", design_swarm, {"orch": "w1:p3", "dock": True, "unseen": dict(MARKS)}, 46),
        ("3a", design_swarm, {"orch": "w1:p3", "dock": True}, 46),
        ("narrow", design_swarm, {"orch": "w1:p1", "thread": "t595", "focus": 1}, 70),
        ("5d", deep_swarm, {"orch": "w1:p1", "focus": 2, "unseen": {"b1": "fail"}}, 160),
        ("demo 2a", demo_swarm, {"orch": "w1:p2", "thread": "wf:wf_search", "agent": "j1a2b3c", "focus": 2}, 160),
        ("demo narrow", demo_swarm, {"orch": "w1:p1", "focus": 1}, 70),
    ]


@pytest.mark.parametrize("name,swarm,kw,w", layouts(), ids=[x[0] for x in layouts()])
@pytest.mark.parametrize("f", FRACTIONS)
@pytest.mark.parametrize("start", ["same frame", "frame 0"])
def test_restyling_equals_a_full_render(name, swarm, kw, w, f, start):
    """The animation tick's contract: a screen drawn once and restyled to ``f`` (within its frame,
    or frames later: spinners and sweeps too) is cell for cell the screen a full render at ``f``
    would draw."""
    whole = render.render(swarm(), render.View(frame=int(f) if start == "same frame" else 0, **kw), w, 45)
    assert whole.animated(), "nothing animated in this layout: the test would be vacuous"
    before = dump(whole)
    spans = render.restyle(whole, f)
    after = dump(whole)
    full = render.render(swarm(), render.View(frame=f, **kw), w, 45)
    assert after == dump(full)
    # every cell that changed is inside a repainted span (else the terminal keeps the old cell)
    covered = {(y, x) for y, x0, x1 in spans for x in range(x0, x1)}
    moved = {(y, x) for y in range(45) for x in range(w) if before[y][x] != after[y][x]}
    assert moved <= covered, sorted(moved - covered)[:5]
    if kw.get("unseen") or 1 < f % 4 < 2 or 3 < f % 4:  # a pulse, or a blink easing
        assert spans  # something moved between the frames


def test_whole_frames_are_the_stepped_design_frames():
    """At whole frames the blink is on, on, off, off (the design's floor(F / 2) % 2) and the
    pulse is the design's 12-step wave, exactly."""
    assert [render.blink(f) for f in range(8)] == [1, 1, 0, 0, 1, 1, 0, 0]
    assert [render.blink_col(ST[FAIL], f) for f in range(4)] == [ST[FAIL], ST[FAIL], FAINT, FAINT]
    old = [(1 - math.cos(2 * math.pi * (f % 12) / 12)) / 2 for f in range(24)]
    assert [render.wave(f) for f in range(24)] == old


def test_colours_glide_in_small_steps_at_the_colour_tick():
    """Between two colour ticks (COLOUR_SECONDS) no animated colour jumps by more than a few
    levels per channel: a gradient, not the 4 fps steps."""
    step = tui.COLOUR_SECONDS / tui.FRAME_SECONDS  # in frames
    ticks = [i * step for i in range(int(12 / step) + 1)]
    for fn, most, levels in ((lambda f: render.pulse_col(("fail", False), render.LINE, f), 10, 25),
                             (lambda f: render.pulse_tint(("wait", False), render.SEL, f), 10, 12),
                             (lambda f: render.blink_col(ST[FAIL], f), 60, 5)):  # the blink eases over a frame
        cols = [rgb(fn(f)) for f in ticks]
        jump = max(max(abs(a - b) for a, b in zip(p, q)) for p, q in itertools.pairwise(cols))
        assert jump <= most, jump
        assert len(set(cols)) > levels  # in-between colours, not two or three levels
    # the old 4 fps blink jumped the whole way at once
    assert max(abs(a - b) for a, b in zip(rgb(ST[FAIL]), rgb(FAINT))) > 150


def test_a_still_view_has_nothing_to_restyle():
    from herdr_panorama.model import DONE, Swarm
    sw = demo_swarm()
    for o in sw.orchestrators:
        o.herdr_status, o.inbox = "idle", []
        for a in o.agents():
            a.status = DONE
    scr = render.render(sw, render.View(), 160, 45)
    assert not scr.animated() and not scr.smooth() and render.restyle(scr, 0.5) == []
    assert not render.animated(Swarm(orchestrators=sw.orchestrators))
    sw.orchestrators[0].agents()[0].status = "run"  # only a spinner and a sweep: whole frames only
    sw.orchestrators[0].herdr_status = "working"
    scr = render.render(sw, render.View(), 160, 45)
    assert scr.animated() and not scr.smooth() and render.restyle(scr, 0.5) == []


async def test_the_animation_tick_restyles_only_animated_cells_never_renders(fake_herdr, monkeypatch):
    """Between frames the tick repaints only the animated cells, without a render; at a whole
    frame (every FRAME_SECONDS) spinners and sweeps step the same way."""
    clock = {"t": 1000.0}
    app, _ = make_app(design_swarm(), focus_pane="w1:p1")
    app.anim_clock = lambda: clock["t"]
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.view.unseen = dict(MARKS)
        app.repaint()
        renders = {"n": 0}
        real = tui.render.render

        def counting(*a, **k):
            renders["n"] += 1
            return real(*a, **k)

        monkeypatch.setattr(tui.render, "render", counting)
        dirty: list = []
        monkeypatch.setattr(app.grid, "refresh", lambda *r, **k: dirty.append(r))
        app._next_frame = clock["t"] + tui.FRAME_SECONDS
        frame0 = int(app.view.frame)
        for i in range(1, 5):  # four colour ticks inside one frame
            clock["t"] += tui.COLOUR_SECONDS
            app.animate()
        assert renders["n"] == 0 and int(app.view.frame) == frame0 and app.view.frame > frame0
        cells = {(y, x) for y, x, _ in app.grid.screen_data.animated()}
        assert dirty and all(r for r in dirty)  # regions, never a whole-widget refresh
        for regions in dirty:
            for r in regions:
                assert r.height == 1
                assert {(r.y, x) for x in range(r.x, r.x + r.width)} & cells
        painted = sum(r.width for regions in dirty for r in regions) / len(dirty)
        assert painted < 160 * 45 / 10  # a small part of the screen per tick
        clock["t"] = app._next_frame  # the next whole frame: spinners and sweeps step, still no render
        dirty.clear()
        app.animate()
        assert renders["n"] == 0 and app.view.frame == frame0 + 1 and dirty
        assert dump(app.grid.screen_data) == dump(real(app.swarm, app.view, 160, 45))
        await pilot.pause()


async def test_a_frame_or_a_refresh_repaints_only_what_changed(fake_herdr, monkeypatch):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        dirty: list = []
        monkeypatch.setattr(app.grid, "refresh", lambda *r, **k: dirty.append(r))
        app.tick()
        assert len(dirty) == 1 and dirty[0]  # regions only
        assert 0 < len({r.y for r in dirty[0]}) < 45
        dirty.clear()
        app.hover(("orch", ("w1:p2",)))  # a re-render: only the hovered card's cells differ
        assert len(dirty) == 1 and dirty[0] and len({r.y for r in dirty[0]}) <= 6


@pytest.mark.parametrize("seed", range(40))
def test_random_restyles_repaint_every_changed_cell(seed):
    """The review's probe: random pairs of times (glyphs stepping while a pulse colour repeats)."""
    import random
    rnd = random.Random(seed)
    name, swarm, kw, w = rnd.choice(layouts())
    a = rnd.choice([0, 0.5, 2.25, 5.5, 6.0, 11.5, 11.95])
    b = a + rnd.choice([0.05, 0.2, 0.25, 1.0, 1.05, 3.0])
    scr = render.render(swarm(), render.View(frame=a, **kw), w, 45)
    before = dump(scr)
    spans = render.restyle(scr, b)
    after = dump(scr)
    covered = {(y, x) for y, x0, x1 in spans for x in range(x0, x1)}
    moved = {(y, x) for y in range(45) for x in range(w) if before[y][x] != after[y][x]}
    assert moved <= covered, (name, a, b, sorted(moved - covered)[:5])


def test_the_textual_crop_patch_is_on_for_the_verified_textual_only():
    """tui patches Textual's ChopsUpdate (a private API). Verified against Textual 8.2; a newer
    Textual must be re-verified (the tests below) before TEXTUAL_CROP_VERIFIED is widened."""
    import textual
    from textual._compositor import ChopsUpdate
    assert textual.__version__.startswith(tui.TEXTUAL_CROP_VERIFIED), (
        f"Textual {textual.__version__}: re-verify tui._render_segments against its ChopsUpdate, then update "
        "TEXTUAL_CROP_VERIFIED and the pin in pyproject.toml")
    assert ChopsUpdate.render_segments is tui._render_segments


def test_the_crop_patch_falls_back_to_textual_on_anything_unexpected():
    from rich.console import Console

    class Odd:  # a changed internal layout: no chops/chop_ends
        spans = [(0, 1, 2)]

    calls = []
    orig = tui._textual_render_segments
    try:
        tui._textual_render_segments = lambda self, console: calls.append(self) or "fallback"
        assert tui._render_segments(Odd(), Console()) == "fallback" and calls
    finally:
        tui._textual_render_segments = orig


def test_the_crop_never_starts_inside_a_wide_character():
    from rich.console import Console
    from rich.segment import Segment
    from textual._compositor import ChopsUpdate
    from textual.strip import Strip
    strip = Strip([Segment("ab漢cd")])  # 漢 holds columns 2 and 3
    upd = ChopsUpdate([{0: strip}], [(0, 3, 6)], [[6]])
    out = upd.render_segments(Console(force_terminal=True, color_system="truecolor"))
    assert out == "\x1b[1;3H漢cd"


def test_textual_partial_updates_send_only_the_changed_cells():
    """tui patches Textual's ChopsUpdate: a span in the middle of a line is sent from its own
    column, not from the line's start."""
    from rich.console import Console
    from rich.segment import Segment
    from textual._compositor import ChopsUpdate
    from textual.strip import Strip
    strip = Strip([Segment("a" * 10), Segment("B" * 5), Segment("c" * 10)])
    upd = ChopsUpdate([{0: strip}], [(0, 12, 14)], [[25]])
    out = upd.render_segments(Console(force_terminal=True, color_system="truecolor"))
    assert out == "\x1b[1;13HBB"


async def test_a_low_animation_fps_keeps_spinners_at_their_speed(fake_herdr, tmp_path):
    """animation_fps = 2: the tick comes every 500 ms, so each one moves two whole frames (steppier,
    not slower); the timer runs at the configured rate."""
    d = tmp_path / "plugin-config"
    d.mkdir(exist_ok=True)
    (d / "config.toml").write_text("animation_fps = 2\n")
    assert tui.SwarmApp.colour_seconds() == 0.5
    clock = {"t": 1000.0}
    app, _ = make_app(design_swarm(), focus_pane="w1:p1")
    app.anim_clock = lambda: clock["t"]
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app._next_frame = clock["t"]
        f0 = int(app.view.frame)
        for _ in range(4):
            app.animate()
            clock["t"] += 0.5
        assert int(app.view.frame) == f0 + 1 + 2 + 2 + 2  # 1.75 s of frames, minus the first tick's lead
        assert dump(app.grid.screen_data) == dump(render.render(app.swarm, app.view, 160, 45))


# -- the design between whole frames (smooth: docs/design/render-smooth.mjs, swarm-smooth.js) ----------

SMOOTH = json.loads((FIX / "design_smooth.json").read_text())["frames"]


@pytest.mark.parametrize("frame", sorted(SMOOTH, key=float))
def test_3a_glides_exactly_like_the_design_between_frames(frame):
    """The whole 46-column panel at a fractional frame, cell for cell: tints, pulsing and
    blinking glyphs mid-way."""
    v = render.View(orch="w1:p3", dock=True, frame=float(frame), unseen=dict(MARKS))
    v.hover = ("orch", ("w1:p4",))
    got = cells(render.render(design_swarm(), v, 46, 45), 46)
    want = grid(SMOOTH[frame]["P"])
    diffs = [(x, y, want[y][x], got[y][x]) for y in range(45) for x in range(46) if want[y][x] != got[y][x]]
    assert not diffs, f"{len(diffs)} cells differ, first: {diffs[:5]}"


@pytest.mark.parametrize("frame", sorted(SMOOTH, key=float))
def test_2a_glides_like_the_design_between_frames(frame):
    """The pulsed cells (unseen vs none) and the blinking edges at a fractional frame match the
    design's colours, where the design and the app draw the same character."""
    f = float(frame)
    d_on, d_off = grid(SMOOTH[frame]["G"]), grid(SMOOTH[frame]["N"])
    a_on = cells(render.render(design_swarm(), render.View(orch="w1:p1", thread="t595", agent="a2", focus=2,
                                                           frame=f, unseen=dict(MARKS)), 160, 45), 160)
    a_off = cells(render.render(design_swarm(), render.View(orch="w1:p1", thread="t595", agent="a2", focus=2,
                                                            frame=f), 160, 45), 160)
    same = {(x, y) for y in range(45) for x in range(160) if d_off[y][x][0] == a_off[y][x][0]}
    want = {(x, y): d_on[y][x] for x, y in same if d_on[y][x] != d_off[y][x]}
    got = {(x, y): a_on[y][x] for x, y in same if a_on[y][x] != a_off[y][x]}
    assert len(want) > 400 and want == got
    # the blinking left edges (┃ in an attention colour, no pulse) glide the same way
    edges = [(x, y) for x, y in same if d_off[y][x][0] == "┃"]
    assert edges and all(d_off[y][x] == a_off[y][x] for x, y in edges)
    blinking = {d_off[y][x][1] for x, y in edges} - {ST[s] for s in ST} - {render.FAINT}
    if 1 < f % 4 < 2 or 3 < f % 4:
        assert blinking  # mid-fade: in-between colours, not on or off
