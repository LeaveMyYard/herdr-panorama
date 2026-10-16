"""Mouse: clicks on cards, filters, the + row and action labels, the arrival rule, the wheel
and the hover highlight, in both layouts (pilot clicks on the real widget)."""

import time

from herdr_panorama import render
from herdr_panorama.model import Orchestrator
from herdr_panorama.tui import NewAgentForm

from .demo import demo_swarm
from .test_tui import loaded, make_app, sel

WIDE, NARROW = (160, 45), (70, 45)


def spot(app, kind, ident=None):
    """A cell where a click hits exactly this target (not one drawn inside it)."""
    scr = app.grid.screen_data
    for x0, x1, y, k, i in scr.targets:
        if k == kind and (ident is None or i == ident):
            for x in range(x0, x1):
                if scr.target_at(x, y) == (k, i):
                    return x, y
    return None


def settle(app):
    """As a second later: no target counts as just arrived, no action key is armed-out."""
    app.arrived = {k: v - 5 for k, v in app.arrived.items()}
    app.armed_at = 0.0


async def click(app, pilot, kind, ident=None):
    x, y = spot(app, kind, ident)
    await pilot.click(app.grid, offset=(x, y))
    await pilot.pause()


async def test_wide_cards_select_and_a_second_click_goes_to_the_pane(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        await click(app, pilot, "orch", ("w1:p2",))
        assert sel(app) == (0, "w1:p2", "wf:wf_search", "j1a2b3c")
        await click(app, pilot, "orch", ("w1:p1",))
        assert sel(app) == (0, "w1:p1", "ref:#42", "a1c3d90")
        await click(app, pilot, "thread", ("w1:p1", "ref:#44"))
        assert sel(app) == (1, "w1:p1", "ref:#44", "b3a9c41")
        await click(app, pilot, "thread", ("w1:p1", "ref:#44"))  # a thread has no pane: still just selected
        assert sel(app) == (1, "w1:p1", "ref:#44", "b3a9c41") and fake_herdr.calls_of("agent", "focus") == []
        await click(app, pilot, "agent", ("w1:p1", "ref:#44", "b3a9c41"))
        assert sel(app) == (2, "w1:p1", "ref:#44", "b3a9c41")
        assert fake_herdr.calls_of("agent", "focus") == []  # selecting never acts
        await click(app, pilot, "agent", ("w1:p1", "ref:#44", "b3a9c41"))  # selected: g
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]
    assert app.return_code is not None  # a popup closes after g


async def test_a_selected_orchestrator_clicked_again_goes_to_its_pane(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        await click(app, pilot, "orch", ("w1:p1",))
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]


async def test_a_selected_card_without_focus_is_focused_first(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        await pilot.press("right")  # focus on the threads; the orchestrator stays selected
        settle(app)
        await click(app, pilot, "orch", ("w1:p1",))
        assert sel(app)[0] == 0 and fake_herdr.calls_of("agent", "focus") == []


async def test_pods_in_a_thread_card_select_their_agent(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        x0, x1, y, k, ident = next(t for t in app.grid.screen_data.targets
                                   if t[3] == "agent" and t[4][1] == "ref:#44" and t[1] - t[0] == 3)
        await pilot.click(app.grid, offset=(x0 + 1, y))
        assert sel(app) == (2, "w1:p1", "ref:#44", ident[2])


async def test_complete_and_finished_cards_fold_like_enter(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        await click(app, pilot, "done", ("w1:p1",))
        assert app.view.thread == render.DONE_CARD + "w1:p1" and "w1:p1" in app.view.done_open
        assert "ref:#40" in render.nav(app.swarm, app.view)[1]
        await click(app, pilot, "done", ("w1:p1",))
        assert "w1:p1" not in app.view.done_open
        await click(app, pilot, "thread", ("w1:p1", "ref:#44"))
        await click(app, pilot, "fin", ("w1:p1", "ref:#44"))
        assert app.view.agent == render.FIN_CARD + "ref:#44" and "ref:#44" in app.view.fin_open
        assert sel(app)[0] == 2
        await click(app, pilot, "fin", ("w1:p1", "ref:#44"))
        assert "ref:#44" not in app.view.fin_open


async def test_header_filters_switch_on_click(fake_herdr):
    app, _ = make_app(focus_pane="w1:p2")
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        await click(app, pilot, "filter", ("needs",))
        assert app.view.filter == "needs" and app.view.orch == "w1:p1"
        await click(app, pilot, "filter", ("running",))
        assert app.view.filter == "running"
        await click(app, pilot, "filter", ("all",))
        assert app.view.filter == "all"


async def test_new_agent_row_opens_the_form_only_when_armed(fake_herdr):
    app, _ = make_app()
    app.home_mode = True
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        app.armed_at = time.monotonic() + 30  # right after a background change
        await click(app, pilot, "new", ())
        assert not isinstance(app.screen, NewAgentForm)
        app.armed_at = 0.0
        await click(app, pilot, "new", ())
        assert isinstance(app.screen, NewAgentForm)
        assert app.view.orch == render.NEW_ROW
        before = sel(app)
        await pilot.click(app.screen, offset=(3, 30))  # outside the form: never reaches the cards
        await pilot.pause()
        assert sel(app) == before and isinstance(app.screen, NewAgentForm)


async def test_action_labels_act_on_their_agent(fake_herdr, monkeypatch):
    app, _ = make_app()
    app.home_mode = True
    notes = []
    monkeypatch.setattr(app, "notify", lambda msg, **kw: notes.append((kw.get("title"), msg)))
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        await pilot.press("right", "right")
        settle(app)
        ident = ("w1:p1", "ref:#42", "a1c3d90")
        await click(app, pilot, "transcript", ident)
        assert notes and notes[-1][0] == "transcript"
        app.armed_at = time.monotonic() + 30
        await click(app, pilot, "goto", ident)  # not armed: ignored, like g
        assert fake_herdr.calls_of("agent", "focus") == []
        app.armed_at = 0.0
        await click(app, pilot, "goto", ident)
        assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]


async def test_a_click_on_empty_space_does_nothing(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        before = sel(app), app.view.filter
        scr = app.grid.screen_data
        empty = [(x, y) for y in range(scr.h) for x in range(scr.w) if scr.target_at(x, y) is None]
        for x, y in (empty[0], empty[len(empty) // 2], empty[-1]):
            await pilot.click(app.grid, offset=(x, y))
        await pilot.pause()
        assert (sel(app), app.view.filter) == before
        assert fake_herdr.calls_of("agent", "focus") == [] and app.is_running


async def test_a_vanished_identity_is_not_acted_on(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        before = sel(app)
        sw = demo_swarm()
        sw.orchestrators = [o for o in sw.orchestrators if o.pane_id != "w1:p2"]
        app.swarm = sw  # model swapped, screen not yet repainted: the stale card must not act
        app.click_target("orch", ("w1:p2",))
        app.click_target("thread", ("w1:p2", "wf:wf_search"))
        for kind in ("agent", "goto", "answer", "transcript"):
            app.click_target(kind, ("w1:p1", "ref:#42", "gone"))
        app.click_target("fin", ("w1:p1", "ref:#nope"))
        app.click_target("done", ("w1:p2",))
        await pilot.pause()
        assert sel(app) == before and not app.view.done_open and not app.view.fin_open
        assert fake_herdr.calls_of("agent", "focus") == [] and app.is_running


# -- the arrival rule ------------------------------------------------------------------

async def test_first_paint_targets_ignore_clicks_for_a_second(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        app.armed_at = 0.0
        await click(app, pilot, "orch", ("w1:p2",))  # just appeared
        assert app.view.orch == "w1:p1"
        settle(app)
        await click(app, pilot, "orch", ("w1:p2",))
        assert app.view.orch == "w1:p2"


async def test_a_card_that_moved_in_a_refresh_ignores_clicks(fake_herdr):
    app, holder = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        x, y = spot(app, "orch", ("w1:p3",))
        sw = demo_swarm()
        sw.orchestrators.insert(0, Orchestrator(pane_id="w1:p0", tab_id="w1:t0", tab_label="New one",
                                                session_id="s0", herdr_status="idle"))
        app.apply(sw)  # every card moved down one slot; the selected card is unchanged
        assert app.armed()
        under = app.grid.screen_data.target_at(x, y)
        assert under == ("orch", ("w1:p2",))  # another card is drawn where the operator aimed
        await pilot.click(app.grid, offset=(x, y))
        await pilot.pause()
        assert sel(app)[:2] == (0, "w1:p1")  # ignored: it arrived there less than a second ago
        app.apply(sw)  # a refresh where nothing moves does not re-arm
        settle(app)
        await pilot.click(app.grid, offset=(x, y))
        await pilot.pause()
        assert app.view.orch == "w1:p2"  # a second later: the identity drawn there
        assert fake_herdr.calls_of("agent", "focus") == []


async def test_animation_frames_do_not_count_as_arrivals(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        stamps = dict(app.arrived)
        for _ in range(5):
            app.tick()
        app.hover(("orch", ("w1:p2",)))
        assert app.arrived == stamps
        await click(app, pilot, "orch", ("w1:p2",))
        assert app.view.orch == "w1:p2"


# -- wheel -------------------------------------------------------------------------------

async def scroll(app, pilot, x, y, down=True):
    from textual import events
    cls = events.MouseScrollDown if down else events.MouseScrollUp
    app.grid.post_message(cls(app.grid, x, y, 0, 1 if down else -1, 0, False, False, False))
    await pilot.pause()


async def test_wheel_scrolls_an_overflowing_column_and_keeps_the_selection(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 16)) as pilot:  # short: the orchestrators overflow
        await loaded(app, pilot)
        settle(app)
        before = sel(app)
        x, y = spot(app, "orch", ("w1:p1",))
        await scroll(app, pilot, x, y)
        assert app.view.offsets[0] == 3 and sel(app) == before
        app.apply(demo_swarm())  # a refresh does not scroll it back
        assert app.view.offsets[0] == 3
        for _ in range(20):
            await scroll(app, pilot, x, y)
        assert 3 < app.view.offsets[0] <= 20  # clamped to the content
        await scroll(app, pilot, x, y, down=False)
        await pilot.press("down")  # a selection change follows the cursor again
        assert not app.view.wheel


async def test_a_click_right_after_a_wheel_scroll_does_not_hit_what_slid_under_the_pointer(fake_herdr):
    """Review M (#21): a wheel scroll stamps the cells whose target it changed, like a refresh
    but for SCROLL_ARM_SECONDS; a click there in that window is ignored."""
    import time as _time
    app, _ = make_app()
    async with app.run_test(size=(160, 16)) as pilot:  # short: the orchestrators overflow
        await loaded(app, pilot)
        settle(app)
        x, y = spot(app, "orch", ("w1:p1",))
        before = app._cells()
        await scroll(app, pilot, x, y)
        after = app._cells()
        moved = next(c for c, hit in after.items() if hit[0] == "orch" and before.get(c) != hit)
        target = after[moved]
        app.click_target(*target, cell=moved)
        assert app.view.orch == "w1:p1"  # ignored
        app.scrolled = {c: _time.monotonic() - 0.01 for c in app.scrolled}
        app.click_target(*target, cell=moved)
        assert app.view.orch == target[1][0]


async def test_wheel_outside_a_column_or_on_a_short_one_does_nothing(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        await scroll(app, pilot, 5, 0)  # the header
        x, y = spot(app, "orch", ("w1:p1",))
        await scroll(app, pilot, x, y)  # fits: clamped to 0
        assert app.view.offsets.get(0, 0) == 0


# -- narrow (side split) -----------------------------------------------------------------

async def test_narrow_clicks(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=NARROW) as pilot:
        await loaded(app, pilot)
        settle(app)
        await click(app, pilot, "thread", ("w1:p1", "ref:#44"))
        assert sel(app)[:3] == (1, "w1:p1", "ref:#44")
        await click(app, pilot, "agent", ("w1:p1", "ref:#44", "b3a9c41"))  # its pod
        assert sel(app) == (2, "w1:p1", "ref:#44", "b3a9c41")
        await click(app, pilot, "done", ("w1:p1",))
        assert "w1:p1" in app.view.done_open
        await click(app, pilot, "orch", ("w1:p2",))
        assert sel(app)[:2] == (0, "w1:p2")
        await click(app, pilot, "filter", ("running",))
        assert app.view.filter == "running"
        settle(app)
        await click(app, pilot, "orch", ("w1:p2",))  # selected and focused: its pane
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p2"]]


async def test_narrow_expanded_agent_and_its_actions(fake_herdr):
    app, _ = make_app()
    app.home_mode = True
    async with app.run_test(size=NARROW) as pilot:
        await loaded(app, pilot)
        settle(app)
        ident = ("w1:p1", "ref:#42", "a1c3d90")
        assert spot(app, "goto", ident) and spot(app, "transcript", ident)
        await click(app, pilot, "agent", ident)
        assert sel(app) == (2, *ident)
        await click(app, pilot, "goto", ident)
        assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]


async def test_narrow_wheel_and_new_row(fake_herdr):
    app, _ = make_app()
    app.home_mode = True
    async with app.run_test(size=(70, 14)) as pilot:
        await loaded(app, pilot)
        settle(app)
        assert spot(app, "new", ()) is None  # below the fold
        for _ in range(30):
            await scroll(app, pilot, 10, 4)
        settle(app)
        await click(app, pilot, "new", ())
        assert isinstance(app.screen, NewAgentForm)


# -- hover -------------------------------------------------------------------------------

async def test_hover_highlights_one_card_and_repaints_only_on_change(fake_herdr, monkeypatch):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        x, y = spot(app, "orch", ("w1:p2",))
        x0 = next(t[0] for t in app.grid.screen_data.targets if t[3:] == ("orch", ("w1:p2",)))
        top = min(t[2] for t in app.grid.screen_data.targets if t[3:] == ("orch", ("w1:p2",)))
        assert app.grid.screen_data.c[top][x0].fg == render.LINE
        paints = []
        real = app.repaint
        monkeypatch.setattr(app, "repaint", lambda: (paints.append(1), real()))
        await pilot.hover(app.grid, offset=(x, y))
        await pilot.hover(app.grid, offset=(x + 1, y))
        await pilot.hover(app.grid, offset=(x + 2, y + 1))  # the same card: no repaint
        assert app.view.hover == ("orch", ("w1:p2",)) and len(paints) == 1
        assert app.grid.screen_data.c[top][x0].fg == render.HOVER
        hit = app.grid.screen_data.target_at(x0 + 2, y)
        assert hit is not None
        await pilot.hover(app.screen, offset=(80, 0))  # the header: no card
        assert app.view.hover is None and len(paints) == 2


# -- review of #7 ------------------------------------------------------------------------

async def test_r7_m1_pods_that_shifted_ignore_clicks_even_if_their_agent_card_did_not_move(fake_herdr):
    from herdr_panorama.model import FAIL
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        before = sel(app)
        assert before == (0, "w1:p1", "ref:#42", "a1c3d90")
        pods = {(x0, y): ident for x0, x1, y, k, ident in app.grid.screen_data.targets
                if k == "agent" and x1 - x0 == 3 and ident[1] == "ref:#42"}
        sw = demo_swarm()
        next(a for a in sw.orchestrators[0].threads[0].agents if a.id == "a3e1b22").status = FAIL
        app.apply(sw)  # the failed agent sorts first: every pod shifts one slot
        scr = app.grid.screen_data
        shifted = [(x, y) for (x, y), ident in pods.items() if scr.target_at(x, y) != ("agent", ident)]
        assert any(scr.target_at(x, y) == ("agent", (*before[1:3], "a1c3d90")) for x, y in shifted)
        for x, y in shifted:  # each aimed at one agent before the refresh, now drawn as another
            await pilot.click(app.grid, offset=(x, y))
            await pilot.pause()
            assert sel(app) == before
        assert fake_herdr.calls_of("agent", "focus") == []


async def test_r7_l1_fold_clicks_are_armed_like_enter(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=WIDE) as pilot:
        await loaded(app, pilot)
        settle(app)
        app.armed_at = time.monotonic() + 30  # right after a refresh changed the selected card
        await click(app, pilot, "done", ("w1:p1",))
        assert not app.view.done_open
        app.armed_at = 0.0
        await click(app, pilot, "done", ("w1:p1",))
        assert "w1:p1" in app.view.done_open
