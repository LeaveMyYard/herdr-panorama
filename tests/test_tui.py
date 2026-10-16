"""Pilot tests for the view: navigation, identity cursors, arming, actions, layouts."""

import time

from herdr_panorama import herdr, render
from herdr_panorama.model import DONE, RUN
from herdr_panorama.tui import ARM_SECONDS, Grid, SwarmApp

from .demo import NOW, demo_swarm


def make_app(swarm=None, focus_pane="w1:p1"):
    holder = {"swarm": swarm or demo_swarm()}
    app = SwarmApp(source=lambda: holder["swarm"], live_refresh=False, clock=lambda: NOW, focus_pane=focus_pane)
    return app, holder


async def loaded(app, pilot):
    for _ in range(50):
        await pilot.pause(0.02)
        if app.loaded:
            break
    await pilot.pause()
    return app


def disarm(app):
    app.armed_at = 0.0


def screen_text(app) -> str:
    return app.query_one(Grid).screen_data.text()


def sel(app):
    v = app.view
    return v.focus, v.orch, v.thread, v.agent


async def test_initial_selection_follows_the_invoking_pane(fake_herdr):
    app, _ = make_app(focus_pane="w1:p2")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert sel(app) == (0, "w1:p2", "wf:wf_search", "j1a2b3c")
        assert "search-index-rebuild" in screen_text(app)


async def test_arrow_navigation_across_columns(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert sel(app) == (0, "w1:p1", "ref:#42", "a1c3d90")
        await pilot.press("down")
        assert sel(app) == (0, "w1:p2", "wf:wf_search", "j1a2b3c")
        await pilot.press("up", "right", "down")
        assert sel(app) == (1, "w1:p1", "ref:#44", "b3a9c41")
        await pilot.press("right", "down")
        assert sel(app) == (2, "w1:p1", "ref:#44", render.FIN_CARD + "ref:#44")
        await pilot.press("left", "up")  # back to #42: its remembered agent is restored
        assert sel(app) == (1, "w1:p1", "ref:#42", "a1c3d90")
        await pilot.press("j", "k", "l", "h")  # vim keys too
        assert sel(app) == (1, "w1:p1", "ref:#42", "a1c3d90")


async def test_cursor_tracks_identity_when_a_refresh_reorders(fake_herdr):
    app, holder = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("down")  # Search refactor
        disarm(app)
        sw = demo_swarm()
        sw.orchestrators.reverse()  # a new tab order arrives in the background
        app.apply(sw)
        assert sel(app) == (0, "w1:p2", "wf:wf_search", "j1a2b3c")
        assert app.armed()  # same card: not re-armed
        await pilot.press("down")  # moves relative to the new order
        assert app.view.orch == "w1:p1"


async def test_a_vanished_target_parks_the_cursor(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        disarm(app)
        sw = demo_swarm()
        sw.orchestrators = [o for o in sw.orchestrators if o.pane_id != "w1:p1"]
        app.apply(sw)
        assert sel(app) == (0, None, None, None)
        await pilot.press("g")  # acts on nothing
        assert fake_herdr.calls_of("agent", "focus") == []
        await pilot.press("down")  # arrows resume from the top
        assert app.view.orch == "w1:p2"


async def test_action_keys_are_ignored_right_after_a_background_change(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("g")  # first paint: not armed yet
        assert fake_herdr.calls_of("agent", "focus") == []
        disarm(app)
        sw = demo_swarm()
        sw.orchestrators[0].threads[0].agents[1].status = DONE  # the selected agent changed under the cursor
        app.apply(sw)
        assert not app.armed()
        await pilot.press("g")
        assert fake_herdr.calls_of("agent", "focus") == []
        app.armed_at = time.monotonic() - 0.01
        await pilot.press("g")
        assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]
    assert ARM_SECONDS >= 0.3


async def test_goto_focuses_the_orchestrator_pane_and_closes(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right", "right")
        disarm(app)
        await pilot.press("g")
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]
    assert app.return_code is not None


async def test_goto_failure_keeps_the_view_open(fake_herdr, monkeypatch):
    monkeypatch.setenv("FAKE_HERDR_FAIL", "agent focus")
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        disarm(app)
        await pilot.press("g")
        await pilot.pause()
        assert app.is_running


async def test_answer_opens_the_inbox_after_the_popup_closes(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right", "down", "down", "right")  # PR #48, its asking agent
        assert app.view.agent == "c1d2e3f"
        disarm(app)
        await pilot.press("a")
        await pilot.pause()
    assert fake_herdr.calls_of("plugin") == []  # nothing invoked while the popup is open
    app.after_exit()
    assert fake_herdr.calls_of("plugin", "action", "invoke") == [["plugin", "action", "invoke", herdr.INBOX_ACTION]]


async def test_answer_on_a_blocked_session_goes_to_the_pane(fake_herdr):
    app, _ = make_app(focus_pane="w1:p3")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert sel(app) == (0, "w1:p3", "main:w1:p3", "main:w1:p3")
        assert "! Publish the v2 docs now or after the" in screen_text(app)
        disarm(app)
        await pilot.press("a")
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p3"]]
    assert app.after_exit is None
    assert not any(c[0] in ("agent", "pane") and c[1] in ("prompt", "send-keys", "send-text") for c in fake_herdr.calls())


async def test_no_inbox_means_no_inbox_actions(fake_herdr):
    sw = demo_swarm()
    sw.inbox_ok = False
    app, _ = make_app(sw)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert "i inbox" not in screen_text(app)
        disarm(app)
        await pilot.press("i")
        await pilot.press("right", "down", "down", "right", "a")
        await pilot.pause()
        assert app.is_running and app.after_exit is None


async def test_filter_cycles_and_reselects(fake_herdr):
    app, _ = make_app(focus_pane="w1:p2")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("f")
        assert app.view.filter == "running" and app.view.orch == "w1:p2"
        await pilot.press("f")  # needs me: Search refactor needs nothing -> parked, then first visible
        assert app.view.filter == "needs" and app.view.orch == "w1:p1"
        assert render.nav(app.swarm, app.view)[0] == ["w1:p1", "w1:p3", render.NEW_ROW]
        await pilot.press("f")
        assert app.view.filter == "all"


async def test_enter_unfolds_the_complete_card(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right", "down", "down", "down")
        assert app.view.thread == render.DONE_CARD + "w1:p1"
        disarm(app)
        await pilot.press("enter")
        assert "w1:p1" in app.view.done_open
        await pilot.press("down")
        assert app.view.thread == "ref:#40"


async def test_narrow_layout_in_a_side_split(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(70, 45)) as pilot:
        await loaded(app, pilot)
        text = screen_text(app)
        assert "Billing API  failed · working" in text and "Search refactor  working" in text
        assert "PR #42 invoice export" in text
        await pilot.press("down")
        assert screen_text(app).split("\n")[4].startswith("▌ ")  # focus mark on Search refactor


async def test_refresh_errors_never_kill_the_view(fake_herdr):
    calls = {"n": 0}

    def boom():
        calls["n"] += 1
        raise RuntimeError("parse failure")

    app = SwarmApp(source=boom, live_refresh=False, clock=lambda: NOW)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        assert app.is_running and calls["n"] == 1


async def test_spinner_frames_advance_without_changing_width(fake_herdr):
    sw = demo_swarm()
    app, _ = make_app(sw, focus_pane="w1:p2")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        first = screen_text(app)
        app.tick()
        second = screen_text(app)
        assert first != second
        assert [len(r) for r in first.split("\n")] == [len(r) for r in second.split("\n")]
        assert any(a.status == RUN for a in sw.orchestrators[1].agents())


async def test_m5_a_new_inbox_question_arms_the_answer_key(fake_herdr):
    sw = demo_swarm()
    o = sw.orchestrators[1]  # Search refactor: no questions
    app, _ = make_app(sw, focus_pane="w1:p2")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        disarm(app)
        sw2 = demo_swarm()
        sw2.orchestrators[1].inbox, sw2.orchestrators[1].inbox_texts = [9], ["New?"]
        app.apply(sw2)
        assert not app.armed()  # `a` would now open the inbox: not without the delay
        await pilot.press("a")
        await pilot.pause()
        assert app.is_running and app.after_exit is None
    assert o.inbox == []


async def test_parked_cursor_keys_act_on_nothing(fake_herdr):
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right", "right")
        disarm(app)
        sw = demo_swarm()
        sw.orchestrators[0].threads[0].agents = [a for a in sw.orchestrators[0].threads[0].agents
                                                 if a.id != app.view.agent]
        app.apply(sw)
        assert app.view.agent is None
        disarm(app)
        await pilot.press("g")
        await pilot.press("a")
        await pilot.pause()
        assert fake_herdr.calls_of("agent", "focus") == [] and app.is_running


async def test_h2_collects_never_overlap(fake_herdr):
    """Sequenced, not timed: the first collect is held open while more refreshes fire; none
    of them may start a second collect. Once it is released, one more refresh collects again."""
    import threading
    state = {"now": 0, "peak": 0, "calls": 0}
    lock = threading.Lock()
    entered, release = threading.Event(), threading.Event()

    def slow():
        with lock:
            state["now"] += 1
            state["calls"] += 1
            state["peak"] = max(state["peak"], state["now"])
        entered.set()
        release.wait(5)
        with lock:
            state["now"] -= 1
        return demo_swarm()

    app = SwarmApp(source=slow, live_refresh=False, clock=lambda: NOW)
    async with app.run_test(size=(160, 45)) as pilot:
        for _ in range(100):  # the first collect (on mount) is running and held
            await pilot.pause(0.01)
            if entered.is_set():
                break
        assert entered.is_set()
        for _ in range(8):
            app.refresh_model()  # each finds a collect running: skipped
            await pilot.pause(0.01)
        assert state["calls"] == 1
        release.set()
        for _ in range(100):
            await pilot.pause(0.01)
            if state["now"] == 0 and app.loaded:
                break
        app.refresh_model()  # nothing running now: this one collects
        for _ in range(100):
            await pilot.pause(0.01)
            if state["calls"] == 2 and state["now"] == 0:
                break
    assert state["peak"] == 1 and state["calls"] == 2


async def test_l1_a_failing_collect_keeps_the_last_model_and_says_so(fake_herdr):
    calls = {"n": 0}

    def flaky():
        calls["n"] += 1
        if calls["n"] > 1:
            raise RuntimeError("bad record")
        return demo_swarm()

    app = SwarmApp(source=flaky, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.refresh_model()
        await pilot.pause(0.3)
        assert "Billing API" in screen_text(app)  # last good model kept
        assert "stale" in screen_text(app).split("\n")[0]



# -- animation frames are not background changes ----------------------------------------

async def test_frames_never_recollect_rearm_or_move_the_cursor(fake_herdr):
    calls = {"n": 0}

    def source():
        calls["n"] += 1
        return demo_swarm()

    app = SwarmApp(source=source, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1")
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("right")
        disarm(app)
        before, collects = sel(app), calls["n"]
        for _ in range(12):
            app.tick()
        assert calls["n"] == collects and app.view.frame == 12  # animated from the cached model
        assert sel(app) == before and app.armed()  # not a background change
        await pilot.press("down")  # keys keep working instantly
        assert app.view.thread == "ref:#44"
        await pilot.press("g")
        await pilot.pause()
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]


async def test_frames_keep_running_unfocused_and_pause_when_nothing_moves(fake_herdr):
    """herdr sends focus-out (DECSET 1004) to a pane as soon as the operator focuses another
    one, so Textual's AppBlur means "the operator types next door", not "hidden": the docked
    panel must keep animating. Only a screen where nothing moves skips frames."""
    from textual import events

    from herdr_panorama.model import Swarm
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.post_message(events.AppBlur())
        await pilot.pause()
        assert not app.app_focus  # the blur was delivered
        for _ in range(3):
            app.tick()
        assert app.view.frame == 3
        app.post_message(events.AppFocus())
        await pilot.pause()
        app.tick()
        assert app.view.frame == 4
        app.apply(Swarm(herdr_ok=True))  # nothing animated on screen
        app.tick()
        assert app.view.frame == 4


# -- home tab and + New agent ------------------------------------------------------------

def make_home_app(swarm=None, home_mode=False):
    app = SwarmApp(source=lambda: swarm or demo_swarm(), live_refresh=False, clock=lambda: NOW,
                   focus_pane="w1:p1", home_mode=home_mode)
    return app


async def test_new_row_is_last_stable_and_opens_the_form(fake_herdr):
    from herdr_panorama.tui import NewAgentForm
    app = make_home_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("down", "down", "down", "down")
        assert app.view.orch == render.NEW_ROW and app.view.thread is None
        assert "+  New agent" in screen_text(app)
        disarm(app)
        app.apply(demo_swarm())  # a refresh never moves or changes the + row
        assert app.view.orch == render.NEW_ROW and app.armed()
        await pilot.press("enter")  # armed: the refresh did not change the selected card
        await pilot.pause()
        assert isinstance(app.screen, NewAgentForm)
        assert app.screen.query_one("#cwd").value  # a default directory
        await pilot.press("escape")
        await pilot.pause()
        assert not isinstance(app.screen, NewAgentForm) and app.is_running
        await pilot.press("g", "a")  # nothing to act on from the + row
        assert fake_herdr.calls_of("agent", "focus") == []


async def test_form_submit_is_armed_validated_and_runs_the_flow(fake_herdr, monkeypatch):
    from herdr_panorama import home as home_mod
    from herdr_panorama.tui import NewAgentForm
    seen = []
    monkeypatch.setattr(home_mod, "new_agent", lambda n, c, p: (seen.append((n, c, p)), (home_mod.slugify(n), None))[1])
    app = make_home_app(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("n")
        await pilot.pause()
        form = app.screen
        assert isinstance(form, NewAgentForm)
        await pilot.press("ctrl+s")  # not armed yet: a held key cannot submit
        assert not form.busy and seen == []
        form.armed_at = 0
        await pilot.press("ctrl+s")  # empty name
        assert "at least one letter" in str(form.query_one("#status").render())
        await pilot.press(*"Billing API")
        assert "billing-api" in str(form.query_one("#slug").render())
        await pilot.press("tab", "tab")
        await pilot.press(*"fix it")
        await pilot.press("enter")
        for _ in range(20):
            await pilot.pause(0.05)
            if not isinstance(app.screen, NewAgentForm):
                break
        assert seen and seen[0][0] == "Billing API" and seen[0][2] == "fix it"
        assert not isinstance(app.screen, NewAgentForm) and app.is_running  # home: the view stays


async def test_home_mode_keeps_the_view_open(fake_herdr):
    app = make_home_app(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        disarm(app)
        await pilot.press("enter")  # Enter on an orchestrator: go to its pane
        await pilot.press("escape")  # a stray Esc does not close the home tab
        await pilot.press("right", "right")
        disarm(app)
        await pilot.press("g")
        await pilot.pause()
        assert app.is_running
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]] * 2


def test_new_row_in_narrow_layout():
    v = render.View(orch=render.NEW_ROW, now=NOW)
    text = render.render(demo_swarm(), v, 70, 45).text()
    assert " +   New agent  n" in text


async def test_refreshes_and_frames_keep_running_under_the_form(fake_herdr):
    from herdr_panorama.tui import NewAgentForm
    app = make_home_app(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("n")
        await pilot.pause()
        assert isinstance(app.screen, NewAgentForm)
        app.apply(demo_swarm())  # a background refresh while the form is open
        app.tick()
        await pilot.pause()
        assert app.is_running and isinstance(app.screen, NewAgentForm) and app.view.frame == 1



async def test_r5_m3_submit_freezes_the_fields(fake_herdr, monkeypatch, tmp_path):
    import threading
    from herdr_panorama import home as home_mod
    from herdr_panorama.tui import NewAgentForm
    gate, seen = threading.Event(), []

    def slow(name, cwd, prompt):
        gate.wait(5)
        seen.append((name, cwd, prompt))
        return home_mod.slugify(name), None

    monkeypatch.setattr(home_mod, "new_agent", slow)
    app = make_home_app(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        await pilot.press("n")
        await pilot.pause()
        form = app.screen
        form.armed_at = 0
        form.query_one("#cwd").value = str(tmp_path)
        await pilot.press(*"alpha")
        await pilot.press("ctrl+s")
        form.query_one("#name").value = "beta"  # typed after submit
        form.query_one("#prompt").value = "late"
        gate.set()
        for _ in range(40):
            await pilot.pause(0.05)
            if seen:
                break
    assert seen == [("alpha", str(tmp_path), "")]


async def test_r5_l1_arrows_in_the_form_do_not_move_the_cursor(fake_herdr):
    app = make_home_app(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        before = sel(app)
        await pilot.press("n")
        await pilot.pause()
        await pilot.press("down", "down", "up", "right")
        assert sel(app) == before
