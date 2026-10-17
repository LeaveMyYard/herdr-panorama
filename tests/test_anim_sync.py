"""Animations are a function of the wall clock, not of a per-window counter: every panorama
window (the home view, each tab's panel) shows an animation in the same phase at the same
moment, whenever it started and however long it was hidden (the operator: switching tabs must
not show a phase jump)."""

from __future__ import annotations

import itertools
import math

from herdr_panorama import protocol, render, tui
from herdr_panorama.tui import FRAME_SECONDS

from .anim import AnimClock
from .design_dock import FIX, design_swarm
from .test_smooth import dump
from .test_tui import loaded, make_app
from .test_unseen import MARKS

T = 1_792_000_000.0 + 0.13  # a wall-clock-sized instant, mid-frame (colours between steps)


async def window_at(start: float, tick: float, end: float):
    """A home panorama started at ``start`` whose animation ticks every ``tick`` s until
    ``end``: its frame and its picture then."""
    clock = AnimClock(start)
    app, _ = make_app(design_swarm(), focus_pane="w1:p1")
    app.anim_clock = clock
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.view.unseen = dict(MARKS)  # pulses too, not only spinners and sweeps
        app.repaint()
        while clock.t + tick < end:
            clock.t += tick
            app.animate()
        clock.t = end
        app.animate()
        assert app.grid.screen_data.smooth()  # the picture holds colour glides
        return app.view.frame, dump(app.grid.screen_data), dump(render.render(app.swarm, app.view, 160, 45))


async def test_two_windows_started_apart_show_the_same_phase(fake_herdr):
    """One window up for minutes at 20 fps, one started 2.3 s before ``T`` at 2 fps: at ``T``
    both draw the same frame, glyph for glyph and colour for colour."""
    f1, got1, want1 = await window_at(T - 191.7, tui.COLOUR_SECONDS, T)
    f2, got2, want2 = await window_at(T - 2.3, 0.5, T)
    assert f1 == f2 == T / FRAME_SECONDS
    assert got1 == want1 and got2 == want2  # each is its render at that frame
    assert got1 == got2


async def test_a_panel_and_the_home_view_share_the_frame(tmp_path, fake_herdr):
    """A per-tab panel (a separate process in real use) reads the same frame as the home view
    at the same instant, though it started later and was hidden in between."""
    import json

    from herdr_panorama.panel import PanelApp
    nine = json.loads((FIX / "design_9.json").read_text())["data"]
    clock = AnimClock(T - 60)
    home, _ = make_app(design_swarm(), focus_pane="w1:p1")
    home.anim_clock = clock
    panel = PanelApp(sock_path=tmp_path / "core.sock", pane_id="w1:p9", tab_id="w1:t3", start_client=False)  # never connects
    panel.anim_clock = clock
    async with home.run_test(size=(160, 45)) as pilot:
        await loaded(home, pilot)
        while clock.t < T - 5:
            clock.t += 0.25
            home.animate()
    async with panel.run_test(size=(46, 45)) as pilot:
        await pilot.pause()
        panel.connected = True
        panel._got_state(protocol.state_message(design_swarm(nine), {}, None, ["w1:t1"])["state"])  # hidden
        clock.t = T
        home.animate()
        panel._got_state(protocol.state_message(design_swarm(nine), {}, None, ["w1:t3"])["state"])  # shown
        assert panel.tab_shown and panel.view.frame == home.view.frame == T / FRAME_SECONDS


def test_colour_glides_stay_smooth_at_wall_clock_frames():
    """The frame is about 7e9 (the time of day / 0.25 s): a float64 still resolves ~1e-6 of a
    frame there, so ``frame % period`` feeding a sine needs no extra reduction. A pulse and a
    blink at that size match the same phase near 0, and successive colour ticks move them in
    small steps (no jumps)."""
    base = 600_000_000 * render.PULSE_PERIOD  # 7.2e9, a whole number of pulses and blinks
    assert base % render.BLINK_PERIOD == 0
    for i in range(240):
        x = i * 0.05
        assert math.isclose(render.wave(base + x), render.wave(x), abs_tol=1e-5)
        assert math.isclose(render.blink(base + x), render.blink(x), abs_tol=1e-5)
    step = tui.COLOUR_SECONDS / FRAME_SECONDS

    def steps(f0):
        w = [render.wave(f0 + i * step) for i in range(240)]
        return [b - a for a, b in itertools.pairwise(w)]
    assert all(math.isclose(a, b, abs_tol=1e-5) for a, b in zip(steps(base), steps(0.0)))
