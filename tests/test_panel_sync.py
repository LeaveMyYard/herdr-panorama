"""Per-tab panels, the operator's two reports in per-tab mode:
1. every tab's panel selects the same row, the orchestrator herdr shows (from the core's state,
   not each viewer's own cursor); the hover look is the mouse's alone;
2. the newly shown panel's first frame already has the right focus colour: the switching panel
   has the core announce the focus first (``focusing``), and keeps its own look until it is
   hidden."""

import json
import threading
import time

import pytest

from herdr_panorama import core, herdr, protocol, render, theme

from .design_dock import FIX, design_swarm

NINE = json.loads((FIX / "design_9.json").read_text())


def state(focused=None, visible=None, panels=None):
    return protocol.state_message(design_swarm(NINE["data"]), {}, focused, visible, panels)["state"]


@pytest.fixture
def paths():
    import shutil
    import tempfile
    from pathlib import Path
    base = Path(tempfile.mkdtemp(prefix="ps", dir="/tmp"))
    yield base / "s" / "core.sock", base / "s" / "core.lock"
    shutil.rmtree(base, ignore_errors=True)


def panel(paths, tab, pane):
    from herdr_panorama.panel import PanelApp
    return PanelApp(sock_path=paths[0], pane_id=pane, tab_id=tab, start_client=False)


# -- 1. one selected row everywhere ---------------------------------------------------------

async def test_every_panel_selects_the_orchestrator_herdr_shows(paths):
    a, b = panel(paths, "w1:t2", "w1:p92"), panel(paths, "w1:t4", "w1:p94")
    async with a.run_test(size=(46, 45)) as pa:
        async with b.run_test(size=(46, 45)) as pb:
            for app in (a, b):
                app._got_state(state(visible=["w1:t2"]))
            await pa.pause()
            await pb.pause()
            assert a.view.orch == b.view.orch == "w1:p2"
            for app in (a, b):  # a switch to tab 4: both follow, the hidden one too
                app._got_state(state(visible=["w1:t4"]))
            assert a.view.orch == b.view.orch == "w1:p4"


async def test_a_cursor_moved_to_the_shown_orchestrator_re_arms_the_keys(paths):
    """#75: a state that moves the cursor to the orchestrator herdr now shows changes what Enter,
    g and a act on: the keys wait ARM_SECONDS, as for a refresh that changed the selection; a
    state that leaves the selection as it was does not."""
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(visible=["w1:t2"]))
        await pilot.pause()
        a.monotonic = lambda: 1000.0  # the arming clock held (#76)
        a.armed_at = 0.0
        a._got_state(state(visible=["w1:t2"]))  # the same tab again: nothing moved
        assert a.armed()
        a._got_state(state(visible=["w1:t4"]))  # herdr shows tab 4: the cursor jumps there
        assert a.view.orch == "w1:p4" and not a.armed()
        a.monotonic = lambda: 1000.0 + 0.4
        assert a.armed()


async def test_an_arrow_move_waiting_for_its_switch_is_kept(paths):
    """↑/↓ move from the active row and switch a moment later: a state that still shows the
    same tab (a refresh, a ping) does not pull the cursor back meanwhile."""
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(visible=["w1:t2"]))
        await pilot.pause()
        a.view.orch = "w1:p3"  # the arrow's move, its switch still pending
        a._got_state(state(visible=["w1:t2"]))
        assert a.view.orch == "w1:p3"
        a._got_state(state(visible=["w1:t3"]))  # the switch happened
        assert a.view.orch == "w1:p3"


def test_the_cursor_has_no_highlight_only_the_mouse_has():
    from .test_dock_panel import design_view
    sw = design_swarm()
    v = design_view(4)
    v.cursor_look = False
    others = [o for o in sw.orchestrators if o.pane_id != sw.docked]
    v.orch = others[0].pane_id  # the cursor on a row that is not the docked one
    scr = render.render(sw, v, 46, 45)
    row = next(y for y, r in enumerate(scr.text().splitlines()) if f" {others[0].tab_no} " in r[:5])
    assert scr.c[row][30].bg != render.DOCK_HOV
    v.hover = ("orch", (others[0].pane_id,))  # the mouse over it: the hover look
    scr = render.render(sw, v, 46, 45)
    assert scr.c[row][30].bg == render.DOCK_HOV


# -- 2. the right focus colour on the first frame -------------------------------------------

def test_the_core_announces_a_focus_only_from_a_verified_panel(paths):
    sock, lock = paths
    got = []
    c = core.Core.start(sock, lock, verify_pane=lambda p, pid: p == "w1:p92", on_focusing=got.append)
    import socket
    for claim in ("w1:p92", "w1:p00"):
        s = socket.socket(socket.AF_UNIX)
        s.connect(str(sock))
        s.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "pane_id": claim}})
                  + protocol.encode({"focusing": {"pane_id": "w1:p94"}}))
        time.sleep(0.3)
        s.close()
    assert got == ["w1:p94"]  # the unverified claim's request is ignored
    c.stop()


async def test_a_switch_announces_the_focus_before_showing_the_tab(paths, monkeypatch):
    from herdr_panorama import panel as viewer
    calls = []
    snap = {"panes": [{"pane_id": "w1:p4", "tab_id": "w1:t4"}, {"pane_id": "w1:p94", "tab_id": "w1:t4"}]}
    monkeypatch.setattr(herdr, "snapshot", lambda: snap)
    monkeypatch.setattr(herdr, "focus_tab", lambda t: calls.append(("tab", t)))
    monkeypatch.setattr(herdr, "focus_pane", lambda p: calls.append(("pane", p)))
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a.connected = True
        a._got_state(state(focused="w1:p92", visible=["w1:t2"], panels={"w1:t4": "w1:p94"}))

        def focusing(pane):  # the core's echo: a state naming that pane, as the target gets it
            calls.append(("focusing", pane))
            threading.Timer(0.02, lambda: a.call_from_thread(
                a._got_state, state(focused=pane, visible=["w1:t2"], panels={"w1:t4": "w1:p94"}))).start()
            return True
        monkeypatch.setattr(a.client, "focusing", focusing)
        a._dock("w1:p4", keep_focus=True)
        for _ in range(60):
            await pilot.pause(0.02)
            if not a.docking:
                break
        assert calls == [("focusing", "w1:p94"), ("tab", "w1:t4"), ("pane", "w1:p94")]
    assert viewer.PREPAINT_WAIT >= 0.1


async def test_the_switching_panel_keeps_its_look_until_it_is_hidden(paths):
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(focused="w1:p92", visible=["w1:t2"]))
        await pilot.pause()
        assert a.pane_focused
        a.docking = True  # its switch is under way: the core already names the target
        a._got_state(state(focused="w1:p94", visible=["w1:t2"]))
        await pilot.pause()
        assert a.pane_focused and a.grid.screen_data.c[30][20].bg == theme.FIXED[0]
        a._docked(None)  # done: the tab is shown elsewhere now
        await pilot.pause()
        assert not a.pane_focused


async def test_a_hidden_target_paints_focused_before_it_is_shown(paths):
    b = panel(paths, "w1:t4", "w1:p94")
    async with b.run_test(size=(46, 45)) as pilot:
        b._got_state(state(focused="w1:p92", visible=["w1:t2"]))
        await pilot.pause()
        assert b.grid.screen_data.c[30][20].bg == theme.FIXED[1]
        b._got_state(state(focused="w1:p94", visible=["w1:t2"]))  # announced, tab not shown yet
        await pilot.pause()
        assert b.grid.screen_data.c[30][20].bg == theme.FIXED[0]


async def test_the_core_side_publishes_an_announced_focus(paths, fake_herdr):
    from herdr_panorama.tui import SwarmApp
    sw = design_swarm(NINE["data"])
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, home_mode=True, cockpit_mode=False)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        app.focusing("w1:p94")
        assert app.herdr_focused_pane == "w1:p94"


def test_tests_never_reach_a_live_herdr():
    """The suite runs from herdr panes: nothing of the session it runs in may leak in."""
    import os
    assert not os.environ.get("HERDR_SOCKET_PATH") and not os.environ.get("HERDR_PANE_ID")
    assert herdr.socket_call("pane.focus", {"pane_id": "x"}) == (None, "HERDR_SOCKET_PATH is not set")


# -- review of #44: the announcement is provisional ------------------------------------------

async def test_a_failed_switch_takes_its_announcement_back(paths, monkeypatch):
    """focus_tab fails after the announcement: the panel announces itself again, so the core
    stops naming the target and the panel the operator is still in looks focused."""
    calls = []
    snap = {"panes": [{"pane_id": "w1:p4", "tab_id": "w1:t4"}, {"pane_id": "w1:p94", "tab_id": "w1:t4"}]}
    monkeypatch.setattr(herdr, "snapshot", lambda: snap)
    monkeypatch.setattr(herdr, "focus_tab", lambda t: calls.append(("tab", t)) or "tab_not_found")
    monkeypatch.setattr(herdr, "focus_pane", lambda p: calls.append(("pane", p)))
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a.connected = True
        a._got_state(state(focused="w1:p92", visible=["w1:t2"], panels={"w1:t4": "w1:p94"}))

        def focusing(pane):  # the core: echoes every announcement as the focused pane
            calls.append(("focusing", pane))
            threading.Timer(0.01, lambda: a.call_from_thread(
                a._got_state, state(focused=pane, visible=["w1:t2"], panels={"w1:t4": "w1:p94"}))).start()
            return True
        monkeypatch.setattr(a.client, "focusing", focusing)
        a._dock("w1:p4", keep_focus=True)
        for _ in range(60):
            await pilot.pause(0.02)
            if not a.docking and ("focusing", "w1:p92") in calls:
                break
        await pilot.pause(0.1)
        assert calls == [("focusing", "w1:p94"), ("tab", "w1:t4"), ("focusing", "w1:p92")]
        assert a.pane_focused and a.grid.screen_data.c[30][20].bg == theme.FIXED[0]


async def test_the_core_rereads_the_real_focus_after_an_unconfirmed_announcement(paths, fake_herdr, monkeypatch):
    """The announcing panel died, or never took it back: herdr's real focus wins after a moment."""
    from herdr_panorama import tui
    from herdr_panorama.tui import SwarmApp
    monkeypatch.setattr(tui, "FOCUS_CONFIRM_SECONDS", 0.2)
    monkeypatch.setattr(herdr, "snapshot", lambda: {"focused_pane_id": "w1:p92", "panes": []})
    sw = design_swarm(NINE["data"])
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, home_mode=True, cockpit_mode=False)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        app.focusing("w1:p94")
        assert app.herdr_focused_pane == "w1:p94"
        for _ in range(40):
            await pilot.pause(0.05)
            if app.herdr_focused_pane == "w1:p92":
                break
        assert app.herdr_focused_pane == "w1:p92"


async def test_a_confirmed_announcement_is_not_second_guessed(paths, fake_herdr, monkeypatch):
    from herdr_panorama import tui
    from herdr_panorama.tui import SwarmApp
    monkeypatch.setattr(tui, "FOCUS_CONFIRM_SECONDS", 0.2)
    asked = []
    monkeypatch.setattr(herdr, "snapshot", lambda: asked.append(1) or {"focused_pane_id": "w1:p92", "panes": []})
    sw = design_swarm(NINE["data"])
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, home_mode=True, cockpit_mode=False)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        app.focusing("w1:p94")
        app.herdr_focus("w1:p94")  # herdr's own pane_focused event confirms it
        await pilot.pause(0.5)
        assert app.herdr_focused_pane == "w1:p94" and asked == []
