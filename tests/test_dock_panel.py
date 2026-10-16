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


async def test_one_click_on_an_entry_docks_and_question_marks_answer(fake_herdr, monkeypatch):
    """The docked 3a panel: one click switches to that orchestrator (the operator's request),
    and the keyboard stays on panorama (dock_keep_focus, the default)."""
    docked = record_docks(monkeypatch)
    answered = []
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        monkeypatch.setattr(app, "_open_inbox", lambda question=None, pane=None: answered.append(pane))
        await pilot.click(app.grid, offset=spot(app, "orch", ("w1:p2",)))
        assert await wait_for(pilot, lambda: docked)
        assert app.view.orch == "w1:p2" and docked == [("w1:p2", True)]  # one click: selected and docked
        await wait_for(pilot, lambda: not app.docking)
        assert fake_herdr.calls_of("agent", "focus") == []  # the keyboard stays on panorama
        await pilot.click(app.grid, offset=spot(app, "answer-orch", ("w1:p1",)))
        await pilot.pause()
        assert answered == ["w1:p1"] and docked == [("w1:p2", True)]  # ?N answers, it does not dock


async def test_one_click_on_the_docked_entry_focuses_its_agent_like_enter(fake_herdr, monkeypatch):
    docked = record_docks(monkeypatch)
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("down")  # the cursor off the docked entry
        await pilot.click(app.grid, offset=spot(app, "orch", ("w1:p3",)))
        assert await wait_for(pilot, lambda: fake_herdr.calls_of("agent", "focus"))
        assert docked == [("w1:p3", False)]  # the same path as Enter: "already there", then its agent
        assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p3"]]


async def test_one_click_still_obeys_arming_and_identity(fake_herdr, monkeypatch):
    import time as _time

    from herdr_panorama import cockpit
    docked = []
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (docked.append(p), (me, None))[1])
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        app.armed_at = _time.monotonic() + 30  # a refresh just changed the selected entry
        await pilot.click(app.grid, offset=spot(app, "orch", ("w1:p2",)))
        await pilot.pause(0.3)
        assert docked == []  # not armed: it may select, never dock
        app.armed_at = 0.0
        sw = design_swarm()
        sw.orchestrators = [o for o in sw.orchestrators if o.pane_id != "w1:p4"]
        app.swarm = sw  # model changed, screen not yet repainted: the stale entry must not act
        app.click_target("orch", ("w1:p4",), cell=None)
        await pilot.pause(0.3)
        assert docked == []


async def test_the_2a_graph_keeps_select_first(fake_herdr, monkeypatch):
    from herdr_panorama import cockpit
    docked = []
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (docked.append(p), (me, None))[1])
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(160, 45)) as pilot:  # zoomed: the full graph
        await loaded(app, pilot)
        assert not app.view.dock
        await pilot.click(app.grid, offset=spot(app, "orch", ("w1:p2",)))
        await pilot.pause(0.3)
        assert app.view.orch == "w1:p2" and docked == []  # first click: select only
        await pilot.click(app.grid, offset=spot(app, "orch", ("w1:p2",)))
        assert await wait_for(pilot, lambda: docked)  # selected card clicked again: open


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
        app.apply(design_swarm())  # the refresh reports panorama docked next to tab 3
        assert app.view.dock and app.view.focus == 0
        assert not app.armed() and app.armed_at > _time.monotonic() + ARM_SECONDS / 2


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


def test_fit_sizes_the_drawable_width_not_the_rect(monkeypatch):
    from herdr_panorama import cockpit
    calls = []
    monkeypatch.setattr(cockpit.herdr, "layout_export", lambda tab: {
        "type": "split", "direction": "right", "first": {"type": "pane", "pane_id": "me"},
        "second": {"type": "pane", "pane_id": "agent"}})
    monkeypatch.setattr(cockpit.herdr, "pane_layout", lambda pane: {"panes": [
        {"pane_id": "me", "rect": {"x": 0, "width": 100}}, {"pane_id": "agent", "rect": {"x": 100, "width": 200}}]})
    monkeypatch.setattr(cockpit.herdr, "set_split_ratio", lambda tab, path, r: calls.append(r))
    monkeypatch.setattr(cockpit, "pane_frame", None)
    cockpit.fit("me", "t")
    assert calls[-1] == (46 + cockpit.DEFAULT_FRAME) / 300  # the measured herdr 0.9.3 frame
    monkeypatch.setattr(cockpit, "pane_frame", 2)
    cockpit.fit("me", "t")
    assert calls[-1] == 48 / 300


async def test_the_view_measures_its_own_frame(fake_herdr, monkeypatch):
    from herdr_panorama import cockpit
    app = await docked_app(fake_herdr)
    monkeypatch.setattr(cockpit, "pane_frame", None)
    async with app.run_test(size=(43, 45)) as pilot:
        await loaded(app, pilot)
        monkeypatch.setattr("herdr_panorama.tui.herdr.pane_layout",
                            lambda pane: {"panes": [{"pane_id": "w1:p9", "rect": {"x": 0, "width": 46}}]})
        app.measure_frame("w1:p9")
        assert cockpit.pane_frame is None  # alone in its tab: not the docked frame
        monkeypatch.setattr("herdr_panorama.tui.herdr.pane_layout", lambda pane: {"panes": [
            {"pane_id": "w1:p9", "rect": {"x": 0, "width": 46}}, {"pane_id": "w1:p3", "rect": {"x": 46, "width": 200}}]})
        app.measure_frame("w1:p9")
        assert cockpit.pane_frame == 3  # rect 46, drawn at 43


async def test_a_new_frame_measurement_refits_even_at_the_same_tab_width(fake_herdr, monkeypatch):
    from herdr_panorama import cockpit, home
    fits = []
    monkeypatch.setattr(cockpit, "fit", lambda pane, tab, last=None: fits.append(last) or 300)
    monkeypatch.setattr(home, "find", lambda snap, found=None, prefer=None: home.Home("w1:p9", "w1:t3"))
    monkeypatch.setattr("herdr_panorama.tui.herdr.snapshot", lambda: {"panes": [], "tabs": []})
    widths = {"rect": 46}
    monkeypatch.setattr("herdr_panorama.tui.herdr.pane_layout", lambda pane: {"panes": [
        {"pane_id": "w1:p9", "rect": {"x": 0, "width": widths["rect"]}}, {"pane_id": "w1:p3", "rect": {"x": 46}}]})
    monkeypatch.setattr(cockpit, "pane_frame", None)
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(44, 45)) as pilot:
        await loaded(app, pilot)
        app.fit_total = 300
        app._fit()  # frame measured 2 = the fallback: nothing new, the width guard holds
        assert fits[-1] == 300
        widths["rect"] = 47  # herdr's frame turned out to be 3 here
        app._fit()
        assert fits[-1] is None and cockpit.pane_frame == 3  # re-fit with the new frame
# -- latency: no graph flash, optimistic docked state ------------------------------------

async def test_no_graph_flash_while_a_dock_moves_panorama(fake_herdr):
    import time as _time

    from herdr_panorama import cockpit
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(160, 45)) as pilot:  # mid-move: herdr gave half the tab
        await loaded(app, pilot)
        app.docking = True
        app.repaint()
        assert app.view.dock  # still the docked panel, not the wide graph
        app.docking = False
        cockpit.moving_until = _time.monotonic() + 1  # a follow's move under way
        app.repaint()
        assert app.view.dock
        cockpit.moving_until = 0.0
        app.dock_hold_until = 0.0
        app.repaint()
        assert not app.view.dock  # settled and still wide (zoomed): the graph


async def test_a_successful_dock_shows_at_once_and_a_stale_collect_does_not_undo_it(fake_herdr):
    import time as _time
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        assert app.swarm.docked == "w1:p3"
        before = _time.monotonic()
        app.show_docked("w1:p1")  # the dock worker reports a successful move next to Payments UI
        assert app.swarm.docked == "w1:p1" and "▌ 1 " in app.grid.screen_data.text()  # at once
        app.apply(design_swarm(), before)  # a collect that began before the dock: still says p3
        assert app.swarm.docked == "w1:p1"
        sw = design_swarm()
        sw.docked = "w1:p1"
        app.apply(sw, _time.monotonic())  # one from after: herdr's word, the hint is dropped
        assert app.docked_hint is None and app.swarm.docked == "w1:p1"


async def test_the_workers_report_the_dock_as_soon_as_the_move_succeeds(fake_herdr, monkeypatch):
    import threading as _th

    from herdr_panorama import cockpit
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        monkeypatch.setattr(cockpit, "follow", lambda me, fl=None, on_docked=None: (on_docked("w1:p2"), ("w1:p9", "docked"))[1])
        t1 = _th.Thread(target=app._follow_worker)
        t1.start()
        for _ in range(40):
            await pilot.pause(0.02)
            if app.swarm.docked == "w1:p2" and not t1.is_alive():
                break
        assert app.swarm.docked == "w1:p2"
        monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (on_docked(p), ("w1:p9", None))[1])
        t2 = _th.Thread(target=app._dock_worker, args=("w1:p4",))
        t2.start()
        for _ in range(40):
            await pilot.pause(0.02)
            if app.swarm.docked == "w1:p4" and not t2.is_alive():
                break
        assert app.swarm.docked == "w1:p4" and not t2.is_alive()


def test_trace_records_each_herdr_call(tmp_path, monkeypatch):
    from herdr_panorama import herdr
    path = tmp_path / "trace.jsonl"
    monkeypatch.setenv("PANORAMA_TRACE", str(path))
    herdr.call("api", "snapshot")  # fails (no herdr in tests) but is still timed
    herdr.socket_call("pane.layout", {"pane_id": "x"})
    ops = [__import__("json").loads(x)["op"] for x in path.read_text().splitlines()]
    assert ops == ["cli api snapshot", "socket pane.layout"]


async def test_r16b_a_dock_report_under_the_lock_and_quit_both_finish(fake_herdr, monkeypatch, tmp_path):
    """The worker reports the dock while it holds the home lock, and the operator presses q at
    that moment (quit relabels under the same lock): neither may wait for the other."""
    import threading as _th
    import time as _time

    from herdr_panorama import home
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    app = await docked_app(fake_herdr)
    done = {}
    inside = _th.Event()

    def worker():
        with home._lock():
            inside.set()
            _time.sleep(0.05)  # the UI thread is now in quit, waiting for this lock
            t = _time.monotonic()
            app._docked_now("w1:p1")  # the early report, from inside the lock
            done["report"] = _time.monotonic() - t
            _time.sleep(0.2)
        done["worker"] = _time.monotonic()
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        app.my_pane = "w1:p9"
        t = _th.Thread(target=worker, daemon=True)
        t.start()
        inside.wait(2)
        start = _time.monotonic()
        app.action_quit()  # on the UI thread, while the worker holds the lock
        done["quit"] = _time.monotonic()
        t.join(2)
    assert "worker" in done and done["quit"] - start < 1.0 and done["worker"] - start < 1.0
    assert done["report"] < 0.1  # the report never waits for the (blocked) UI thread


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

async def test_a_slow_follow_keeps_the_panel_after_the_holds_expire(fake_herdr):
    """The operator's report: a follow can outlast the 1 s holds (slow herdr calls, the
    settle wait); while it runs the pane may still be half the tab wide: no graph."""
    from herdr_panorama import cockpit
    app = await docked_app(fake_herdr)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.following = True
        app.dock_hold_until = 0.0
        cockpit.moving_until = 0.0  # both holds already ran out
        app.repaint()
        assert app.view.dock
        app.following = False
        app.repaint()
        assert not app.view.dock


def test_a_failed_move_clears_the_move_hold(monkeypatch):
    from herdr_panorama import cockpit, home
    monkeypatch.setattr(cockpit, "_leave", lambda snap, me: None)
    monkeypatch.setattr(cockpit.herdr, "move_pane_to_tab", lambda *a, **k: (None, "boom"))
    out = cockpit._move_next_to({}, home.Home("w1:p9", "w1:t9"), {"pane_id": "w1:p1", "tab_id": "w1:t1"}, None)
    assert out[0] is None and cockpit.moving_until == 0.0  # no 1 s of panel for a move that never happened


async def test_the_resize_fit_skips_while_the_lock_is_busy(fake_herdr, monkeypatch):
    from herdr_panorama import home
    app = await docked_app(fake_herdr)

    def busy(self):
        raise home.LockBusy("held elsewhere")
    monkeypatch.setattr(type(app), "_fit_locked", busy)
    async with app.run_test(size=(46, 45)) as pilot:
        await loaded(app, pilot)
        app._fit()  # must not raise into the worker
