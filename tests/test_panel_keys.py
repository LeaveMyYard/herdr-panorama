"""The per-tab panel's keys and clicks (PanelApp): Enter, g, the settled arrows and one click
switch the chats through ``_dock``, which a panel overrides to show the target's tab. These
paths are shared with the home panorama's code (SwarmApp) and were tested through the moving
cockpit until it went (Q187, review of #83 H1); here they run on the panel itself, its switch
worker recorded."""

import json
import time

import pytest

from herdr_panorama import cockpit, render, theme

from .design_dock import FIX, design_swarm

NINE = json.loads((FIX / "design_9.json").read_text())


def state(visible):
    from herdr_panorama import protocol
    return protocol.state_message(design_swarm(NINE["data"]), {}, None, visible, None)["state"]


@pytest.fixture
def sock(tmp_path):
    return tmp_path / "s" / "core.sock"


def panel(sock, monkeypatch):
    """Tab 3's panel, connected, its switches recorded as (pane, keep_focus) instead of run."""
    from herdr_panorama.panel import PanelApp
    app = PanelApp(sock_path=sock, pane_id="w1:p93", tab_id="w1:t3", start_client=False)
    switched: list = []

    def switch(pane_id, keep_focus):
        switched.append((pane_id, keep_focus))
        app._to_ui(app._docked, None)
    monkeypatch.setattr(app, "_switch_worker", switch)
    return app, switched


async def shown(app, pilot):
    app.connected = True
    app._got_state(state(["w1:t3"]))
    for _ in range(50):
        await pilot.pause(0.02)
        if app.loaded:
            break
    await pilot.pause()
    app.arrived = {k: v - 5 for k, v in app.arrived.items()}
    app.armed_at = 0.0


async def settled(app, pilot):
    for _ in range(40):
        await pilot.pause(0.05)
        if not app.docking:
            return


async def arrow(app, pilot, *keys, wait=0.5):
    await pilot.press(*keys)
    for _ in range(int(wait / 0.05)):
        await pilot.pause(0.05)
    await settled(app, pilot)


def other(app):
    return next(o.pane_id for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)


def spot(app, kind, ident):
    scr = app.grid.screen_data
    for x0, x1, y, k, i in scr.targets:
        if (k, i) == (kind, ident):
            for x in range(x0, x1):
                if scr.target_at(x, y) == (k, i):
                    return x, y
    return None


async def wait_for(pilot, cond, tries=40):
    for _ in range(tries):
        if cond():
            return True
        await pilot.pause(0.05)
    return cond()


# -- Enter and g ------------------------------------------------------------------------------

@pytest.mark.parametrize("key,keep,arrows", [("enter", False, True), ("g", False, True), ("enter", True, False)])
async def test_docked_enter_browses_and_g_goes_into_the_agent(sock, monkeypatch, key, keep, arrows):
    """With ↑/↓ switching the chats (dock_on_arrows, the default), Enter goes into the agent like
    g. With dock_on_arrows off, Enter switches and keeps the keyboard (#14)."""
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: arrows)
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        assert app.view.dock
        target = other(app)
        app._select(0, target)
        await pilot.press(key)
        await settled(app, pilot)
        assert switched == [(target, keep)]


async def test_docked_enter_with_keep_focus_off_focuses_the_agent(sock, monkeypatch):
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: False)
    monkeypatch.setattr(cockpit, "keep_focus_setting", lambda: False)
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        target = other(app)
        app._select(0, target)
        await pilot.press("enter")
        await settled(app, pilot)
        assert switched == [(target, False)]


async def test_the_2a_graph_enter_still_focuses_the_agent(sock, monkeypatch):
    """The full graph (wide, not the docked panel): Enter goes into the agent."""
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(160, 45)) as pilot:
        await shown(app, pilot)
        assert not app.view.dock
        await pilot.press("down", "enter")
        await settled(app, pilot)
        assert switched and all(not keep for _, keep in switched)


# -- the settled arrows -----------------------------------------------------------------------

async def test_arrows_switch_the_chats_once_they_settle(sock, monkeypatch):
    """↓ switches to the highlighted orchestrator after the arrows rest, the keyboard staying on
    the panel; a quick run of arrows switches once, at the last one."""
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        await arrow(app, pilot, "down")
        assert switched == [(app.view.orch, True)]
        await arrow(app, pilot, "up", "up", "up", "down")  # held / quick: one switch, at the last
        assert len(switched) == 2 and switched[-1] == (app.view.orch, True)


async def test_arrows_do_not_dock_the_docked_one_new_agent_or_a_remote(sock, monkeypatch):
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        app._select(0, app.swarm.docked)
        app.action_move(0)  # an arrow that stays on the docked entry
        await arrow(app, pilot)
        assert switched == []
        app._select(0, render.NEW_ROW)
        app.action_move(0)
        await arrow(app, pilot)
        assert switched == []
        remote = next(o for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)
        remote.machine = "build-box"  # on another machine: Enter's hint, never a switch
        app._select(0, remote.pane_id)
        app.action_move(0)
        await arrow(app, pilot)
        assert switched == []


async def test_arrows_off_or_the_2a_graph_never_dock_on_arrows(sock, monkeypatch):
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: False)
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        await arrow(app, pilot, "down")
        assert switched == []
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: True)
    wide, switched = panel(sock, monkeypatch)
    async with wide.run_test(size=(160, 45)) as pilot:
        await shown(wide, pilot)
        assert not wide.view.dock
        await arrow(wide, pilot, "down")
        assert switched == []


@pytest.mark.parametrize("why", ["blurred", "herdr says another pane"])
async def test_an_arrow_dock_is_skipped_when_the_keyboard_left(sock, monkeypatch, why):
    """Review of #26: when the arrows' timer fires, the panel must still have the keyboard."""
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        await pilot.press("down")
        if why == "blurred":
            app.pane_focused = False
        else:
            app.herdr_focused_pane = "w1:p1"  # herdr's last report: the operator is in another pane
        await arrow(app, pilot, wait=0.4)
        assert switched == []


async def test_an_arrow_settling_during_a_dock_runs_after_it_even_if_focus_looks_elsewhere(sock, monkeypatch):
    """Review of #26 (Low): "a switch is running" is checked first, so an arrow that settles
    meanwhile runs after it, not lost, even while the focus checks briefly look elsewhere."""
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        app.docking = True
        app.herdr_focused_pane = "w1:p2"  # mid-switch: herdr reports another pane for a moment
        await arrow(app, pilot, "down", wait=0.4)
        assert switched == [] and app._arrow_again
        app.herdr_focused_pane = app.my_pane
        app._docked(None)  # it finished: the settled arrow switches now
        await arrow(app, pilot, wait=0.4)
        assert switched == [(app.view.orch, True)] and not app._arrow_again


async def test_an_arrow_dock_due_during_a_dock_runs_after_it(sock, monkeypatch):
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        app.docking = True  # a switch is running
        await arrow(app, pilot, "down", wait=0.4)
        assert switched == [] and app._arrow_again
        app._docked(None)
        await arrow(app, pilot, wait=0.4)
        assert switched == [(app.view.orch, True)]


# -- one click --------------------------------------------------------------------------------

async def test_one_click_on_an_entry_docks_and_question_marks_answer(sock, monkeypatch):
    """One click switches to that orchestrator, the keyboard staying on the panel
    (dock_keep_focus, the default); ?N answers and does not switch."""
    app, switched = panel(sock, monkeypatch)
    answered = []
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        monkeypatch.setattr(app, "_open_inbox", lambda question=None, pane=None: answered.append(pane))
        target = other(app)
        await pilot.click(app.grid, offset=spot(app, "orch", (target,)))
        assert await wait_for(pilot, lambda: switched)
        assert app.view.orch == target and switched == [(target, True)]
        await settled(app, pilot)
        asks = [i for _, _, _, k, i in app.grid.screen_data.targets if k == "answer-orch"]
        assert asks
        await pilot.click(app.grid, offset=spot(app, "answer-orch", asks[0]))
        await pilot.pause()
        assert answered == [asks[0][0]] and switched == [(target, True)]


async def test_one_click_on_the_docked_entry_focuses_its_agent_like_enter(sock, monkeypatch):
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        docked = app.swarm.docked
        app._select(0, other(app))  # the cursor off the docked entry
        await pilot.click(app.grid, offset=spot(app, "orch", (docked,)))
        assert await wait_for(pilot, lambda: switched)
        assert switched == [(docked, False)]  # the same path as Enter: into its agent


async def test_one_click_still_obeys_arming_and_identity(sock, monkeypatch):
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        target = other(app)
        app.armed_at = time.monotonic() + 30  # a refresh just changed the selected entry
        await pilot.click(app.grid, offset=spot(app, "orch", (target,)))
        await pilot.pause(0.3)
        assert switched == []  # not armed: it may select, never switch
        app.armed_at = 0.0
        gone = app.swarm.orchestrators[-1].pane_id
        app.swarm.orchestrators = app.swarm.orchestrators[:-1]  # the model changed, not repainted yet
        app.click_target("orch", (gone,), cell=None)
        await pilot.pause(0.3)
        assert switched == []


async def test_the_2a_graph_keeps_select_first(sock, monkeypatch):
    app, switched = panel(sock, monkeypatch)
    async with app.run_test(size=(160, 45)) as pilot:  # wide: the full graph
        await shown(app, pilot)
        assert not app.view.dock
        target = other(app)
        await pilot.click(app.grid, offset=spot(app, "orch", (target,)))
        await pilot.pause(0.3)
        assert app.view.orch == target and switched == []  # first click: select only
        await pilot.click(app.grid, offset=spot(app, "orch", (target,)))
        assert await wait_for(pilot, lambda: switched)  # the selected card clicked again: open


# -- while a switch runs ----------------------------------------------------------------------

async def test_no_graph_flash_while_a_switch_runs(sock, monkeypatch):
    app, _ = panel(sock, monkeypatch)
    async with app.run_test(size=(160, 45)) as pilot:  # herdr gave the pane the whole width a moment
        await shown(app, pilot)
        app.docking = True
        app.repaint()
        assert app.view.dock  # still the docked panel, not the wide graph
        app.docking = False
        app.repaint()
        assert not app.view.dock


async def test_a_switch_that_ends_judges_the_focus_again(sock, monkeypatch):
    """Review of #25: herdr's focus event can arrive while a switch runs; when it ends the
    panel's look is judged again from what herdr last said."""
    app, _ = panel(sock, monkeypatch)
    focused, unfocused = theme.FIXED
    async with app.run_test(size=(46, 45)) as pilot:
        await shown(app, pilot)
        bg = lambda: app.grid.screen_data.c[30][20].bg  # noqa: E731
        app.herdr_focus("w1:p2")
        await pilot.pause()
        assert not app.pane_focused and bg() == unfocused
        app.herdr_focused_pane = app.my_pane
        app._docked(None)
        await pilot.pause()
        assert app.pane_focused and bg() == focused
