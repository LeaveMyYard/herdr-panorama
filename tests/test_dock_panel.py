"""The cockpit's docked panel (design 3a "Compact list", drawn with 9e's tiles: its cell-for-cell
golden is tests/test_tiles.py): its click targets and layout rules."""

import pytest

from herdr_panorama import render

from .design_dock import design_swarm


def design_view(frame: int) -> render.View:
    v = render.View(orch="w1:p3", dock=True, frame=frame)
    v.hover = ("orch", ("w1:p4",))  # the design's hover: Billing Migrations
    return v


def test_every_card_is_a_click_target_and_question_marks_answer():
    scr = render.render(design_swarm(), design_view(4), 46, 45)
    kinds = {(k, i) for _, _, _, k, i in scr.targets}
    assert {("orch", (f"w1:p{n}",)) for n in (1, 2, 3, 4)} <= kinds
    assert ("new", ()) in kinds
    assert ("answer-orch", ("w1:p1",)) in kinds and ("answer-orch", ("w1:p3",)) in kinds
    xq = scr.text().splitlines()[2].index("?1")
    assert scr.target_at(xq, 2) == scr.target_at(xq + 1, 2) == ("answer-orch", ("w1:p1",))  # "?1" on Payments UI
    assert scr.target_at(xq + 3, 2) == ("orch", ("w1:p1",))
    assert scr.target_at(20, 3) == ("orch", ("w1:p1",))  # the second line too
    assert scr.target_at(20, 12) == ("orch", ("w1:p3",))  # the docked one's thread list
    assert scr.target_at(20, 4) is None  # the gap between entries


def test_the_cursor_wears_the_hover_look_and_the_docked_entry_keeps_its_bar():
    v = design_view(4)
    v.hover, v.orch = None, "w1:p2"
    scr = render.render(design_swarm(), v, 46, 45)
    assert scr.c[5][0].ch == "›" and scr.c[6][40].bg == render.DOCK_HOV  # Data Pipeline, the cursor
    assert scr.c[8][0].ch == "▌" and scr.c[8][0].fg == render.KEY and scr.c[8][20].bg == render.SEL  # docked


def test_docked_panel_scrolls_to_the_cursor():
    v = design_view(0)
    v.hover, v.orch = None, render.NEW_ROW
    scr = render.render(design_swarm(), v, 46, 16)
    assert "New agent" in scr.text()
    assert "SWARM" in scr.text().splitlines()[0] and "move" in scr.text().splitlines()[-1]


# -- the view ----------------------------------------------------------------------------

async def docked_app(fake_herdr, cockpit_on=True):
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=design_swarm, live_refresh=False, clock=lambda: 0.0, focus_pane="w1:p3",
                   home_mode=True, cockpit_mode=cockpit_on)
    return app


async def loaded(app, pilot):
    for _ in range(50):
        await pilot.pause(0.02)
        if app.loaded:
            break
    await pilot.pause()
    app.arrived = {k: v - 5 for k, v in app.arrived.items()}
    app.armed_at = 0.0


def spot(app, kind, ident):
    scr = app.grid.screen_data
    for x0, x1, y, k, i in scr.targets:
        if (k, i) == (kind, ident):
            for x in range(x0, x1):
                if scr.target_at(x, y) == (k, i):
                    return x, y
    return None


async def test_docked_view_uses_the_panel_and_keeps_one_column(fake_herdr):
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        assert app.view.dock and "▌ 3 " in app.grid.screen_data.text()  # the docked entry
        await pilot.press("right")
        assert app.view.focus == 0  # one column: → does not leave it
        await pilot.press("down")
        assert app.view.orch == "w1:p4"
        await pilot.press("down")
        assert app.view.orch == render.NEW_ROW


async def test_wide_or_undocked_keeps_the_2a_layout(fake_herdr):
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert not app.view.dock  # wide: the 2a graph
    sw = design_swarm()
    sw.docked = None
    from herdr_panorama.tui import SwarmApp
    app2 = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, focus_pane="w1:p3", home_mode=True)
    async with app2.run_test(size=(46, 45)) as pilot:
        await loaded(app2, pilot)
        assert not app2.view.dock and "SWARM" not in app2.grid.screen_data.text()


async def wait_for(pilot, cond, n=40):
    for _ in range(n):
        await pilot.pause(0.05)
        if cond():
            return True
    return cond()


def record_docks(monkeypatch) -> list:
    """cockpit.dock replaced: each call's (target, focus_me) recorded, panorama 'docked' in place."""
    from herdr_panorama import cockpit
    docked: list = []

    def dock(p, me=None, on_docked=None, focus_me=False):
        docked.append((p, focus_me))
        return me or "w1:p9", None  # panorama's pane after the move
    monkeypatch.setattr(cockpit, "dock", dock)
    return docked


async def test_a_card_that_moved_ignores_clicks(fake_herdr):
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        x, y = spot(app, "orch", ("w1:p2",))
        sw = design_swarm()
        sw.docked = "w1:p1"  # docked elsewhere: Payments UI's card grows, Data Pipeline moves down
        app.apply(sw)
        under = app.grid.screen_data.target_at(x, y)
        assert under == ("orch", ("w1:p1",))  # another card is now where the operator aimed
        await pilot.click(app.grid, offset=(x, y))
        await pilot.pause()
        assert app.view.orch == "w1:p3"  # ignored: something else arrived under the pointer


def test_collector_finds_the_docked_agent_and_tab_numbers(tmp_path):
    from herdr_panorama.collect import Collector
    snap = {"tabs": [{"tab_id": "w1:t1", "label": "a", "workspace_id": "w1"},
                     {"tab_id": "w2:t5", "label": "x", "workspace_id": "w2"},
                     {"tab_id": "w1:t7", "label": "b", "workspace_id": "w1"}],
            "panes": [{"pane_id": "w1:p9", "tab_id": "w1:t7", "workspace_id": "w1", "label": "◆ panorama"}],
            "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude"},
                       {"pane_id": "w1:p7", "tab_id": "w1:t7", "workspace_id": "w1", "agent": "claude"}]}
    c = Collector(root=str(tmp_path), done_window=3600)
    assert c.collect(snap, None).docked is None  # no own pane known: never docked
    c.self_pane = "w1:p9"
    sw = c.collect(snap, None)
    assert sw.docked == "w1:p7"
    assert [(o.pane_id, o.tab_no) for o in sw.orchestrators] == [("w1:p1", 1), ("w1:p7", 2)]


async def test_r11_a_refresh_that_docks_the_view_rearms_the_keys(fake_herdr):
    """A background refresh that switches to the dock panel (Claude started in panorama's
    tab) changes what Enter does, so it re-arms: the next Enter within ARM_SECONDS is ignored."""
    import time as _time

    from herdr_panorama.tui import ARM_SECONDS, SwarmApp
    undocked = design_swarm()
    undocked.docked = None
    holder = {"sw": undocked}
    app = SwarmApp(source=lambda: holder["sw"], live_refresh=False, clock=lambda: 0.0, focus_pane="w1:p3",
                   home_mode=True, cockpit_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right")
        assert not app.view.dock and app.view.focus == 1  # on a thread
        app.armed_at = 0.0
        t0 = _time.monotonic()
        app.apply(design_swarm())  # the refresh reports panorama docked next to tab 3
        assert app.view.dock and app.view.focus == 0
        # re-armed by this apply: measured from before it, so a slow (loaded) run cannot fail it
        assert app.armed_at >= t0 + ARM_SECONDS


# -- the width herdr actually gives the pane (review: clipped "workin", "pfx+spc bac") ----

STATES = ("failed", "blocked", "working", "needs you", "waiting", "stale", "idle", "done")


@pytest.mark.parametrize("w", [36, 40, 44, 46, 50, 60])
def test_3a_lays_out_to_the_width_it_gets(w):
    sw = design_swarm()
    v = design_view(4)
    v.zoom_hint = True
    scr = render.render(sw, v, w, 45)
    lines = scr.text().splitlines()
    for o in sw.orchestrators:  # every state label whole, flush with the real right edge
        row = next(r for r in lines if f" {o.tab_no} " in r[:5])
        assert any(row.rstrip().endswith(st) for st in STATES), (w, row)
        assert len(row.rstrip()) == w - 1
    assert lines[0].rstrip().endswith("?3 !1 ✗1")
    foot = lines[-1].rstrip()
    assert foot.endswith("back") and len(foot) <= w - 1, foot  # never cut; hints dropped instead
    assert "z graph" in lines[-2]
    assert any("New agent" in r and r.rstrip().endswith("n") for r in lines)


def test_3a_at_its_design_width_keeps_every_footer_hint():
    scr = render.render(design_swarm(), design_view(4), 46, 45)
    assert scr.text().splitlines()[-1].strip() == "↑↓ move ret open a answer n new pfx+spc back"


# -- latency: no graph flash, optimistic docked state ------------------------------------

def test_trace_records_each_herdr_call(tmp_path, monkeypatch):
    from herdr_panorama import herdr
    path = tmp_path / "trace.jsonl"
    monkeypatch.setenv("PANORAMA_TRACE", str(path))
    herdr.call("api", "snapshot")  # fails (no herdr in tests) but is still timed
    herdr.socket_call("pane.layout", {"pane_id": "x"})
    ops = [__import__("json").loads(x)["op"] for x in path.read_text().splitlines()]
    assert ops == ["cli api snapshot", "socket pane.layout"]


def test_r16b_the_lock_gives_up_instead_of_hanging(tmp_path, monkeypatch):
    import threading as _th
    import time as _time

    from herdr_panorama import home
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    held, release = _th.Event(), _th.Event()

    def hold():
        with home._lock():
            held.set()
            release.wait(5)
    _th.Thread(target=hold, daemon=True).start()
    held.wait(2)
    start = _time.monotonic()
    with pytest.raises(home.LockBusy):
        with home._lock(timeout=0.3):
            pass
    assert _time.monotonic() - start < 1.0
    start = _time.monotonic()
    assert home.leave("w1:p9").startswith("busy")  # quit's relabel is skipped, not waited for
    assert _time.monotonic() - start < 1.0
    release.set()


# -- final pass of #16 ------------------------------------------------------------------------

