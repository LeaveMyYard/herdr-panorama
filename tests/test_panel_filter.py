"""Per-tab panels, the operator's report: "changing filters on the left panel is not synced
between tabs and navigation does not take current filter into account".

1. The filter is shared view state: a panel's choice goes to the core (``filter``, protocol
   1.3), the home panorama takes it, and every state it publishes carries it (``state.filter``),
   so every tab's panel (and the home view) shows the same filter.
2. Navigation moves only among what the filter shows: the shown orchestrator, when the filter
   hides it, is no selected row; ↑/↓ resume from where it sits to the next shown one in their
   direction (not from the top), and no key acts on a hidden one."""

import copy
import json
import socket
import time

import pytest

from herdr_panorama import core, protocol, render

from .design_dock import FIX, design_swarm

NINE = json.loads((FIX / "design_9.json").read_text())
# the fixture under "needs": w1:p1, w1:p2, w1:p3 shown; w1:p4 and w1:p5 hidden


def state(visible=None, view_filter=None, sw=None):
    st = protocol.state_message(sw or design_swarm(NINE["data"]), {}, None, visible, None)["state"]
    if view_filter is not None:
        st["filter"] = view_filter  # 1.3's field, as the core sends it
    return st


@pytest.fixture
def paths():
    import shutil
    import tempfile
    from pathlib import Path
    base = Path(tempfile.mkdtemp(prefix="pf", dir="/tmp"))
    yield base / "s" / "core.sock", base / "s" / "core.lock"
    shutil.rmtree(base, ignore_errors=True)


def panel(paths, tab, pane, start_client=False):
    from herdr_panorama.panel import PanelApp
    return PanelApp(sock_path=paths[0], pane_id=pane, tab_id=tab, start_client=start_client)


async def settle(pilot, test, tries=100):
    for _ in range(tries):
        if test():
            return True
        await pilot.pause(0.02)
    return test()


# -- 1. one filter everywhere -----------------------------------------------------------

def test_the_core_passes_a_panels_filter_on(paths):
    sock, lock = paths
    got = []
    c = core.Core.start(sock, lock, on_filter=got.append)
    try:
        for role in ("panel", "key"):  # the open key's connection has no view to filter
            s = socket.socket(socket.AF_UNIX)
            s.connect(str(sock))
            s.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "role": role}})
                      + protocol.encode({"filter": {"name": "needs"}})
                      + protocol.encode({"filter": "junk"}))
            time.sleep(0.3)
            s.close()
        assert got == ["needs"]
    finally:
        c.stop()


def test_the_state_carries_the_filter_and_the_version_says_so():
    assert protocol.PROTOCOL == "1.3" and protocol.compatible("1.2")
    sw = design_swarm(NINE["data"])
    assert protocol.state_message(sw, {}, None, None, None, "running")["state"]["filter"] == "running"
    assert "filter" not in protocol.state_message(sw, {}, None, None)["state"]  # none given: left out


async def test_a_filter_chosen_in_one_panel_shows_in_every_panel(paths):
    """The real socket end to end: f in tab 2's panel; tab 4's panel shows "needs" too."""
    sock, lock = paths
    sw = design_swarm(NINE["data"])
    c = None

    def chosen(name):  # the home panorama's part (tui.SwarmApp._filter_from_panel)
        c.publish(protocol.state_message(sw, {}, None, ["w1:t2"], None, name))
    c = core.Core.start(sock, lock, verify_pane=lambda p, pid: True, on_filter=chosen)
    try:
        c.publish(protocol.state_message(sw, {}, None, ["w1:t2"], None, "all"))
        a = panel(paths, "w1:t2", "w1:p92", start_client=True)
        b = panel(paths, "w1:t4", "w1:p94", start_client=True)
        async with a.run_test(size=(46, 45)) as pa, b.run_test(size=(46, 45)) as pb:
            assert await settle(pa, lambda: a.connected and b.connected and a.loaded and b.loaded)
            await pa.press("f")  # all -> running
            await pa.press("f")  # running -> needs
            assert a.view.filter == "needs"
            assert await settle(pb, lambda: b.view.filter == "needs"), b.view.filter
            assert a.view.filter == "needs"
    finally:
        c.stop()


async def test_the_home_panorama_takes_a_panels_filter_and_publishes_it(fake_herdr):
    from herdr_panorama.tui import SwarmApp
    sw = design_swarm(NINE["data"])
    sent = []

    class FakeCore:
        def publish(self, msg):
            sent.append(msg)
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, home_mode=True, cockpit_mode=False)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        app.core = FakeCore()
        app._filter_from_panel("needs")
        assert app.view.filter == "needs" and sent[-1]["state"]["filter"] == "needs"
        app._filter_from_panel("bogus")  # not a filter: nothing changes
        assert app.view.filter == "needs"
        await pilot.press("f")  # the home view's own choice goes to the panels as well
        assert app.view.filter == "all" and sent[-1]["state"]["filter"] == "all"
        app.core = None


async def test_a_panel_keeps_its_choice_against_an_older_state(paths, monkeypatch):
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(visible=["w1:t2"], view_filter="all"))
        await pilot.pause()
        monkeypatch.setattr(a.client, "filter", lambda name: True)  # sent to the core
        a._set_filter("needs")
        a._got_state(state(visible=["w1:t2"], view_filter="all"))  # from before the core had it
        assert a.view.filter == "needs"
        a._got_state(state(visible=["w1:t2"], view_filter="needs"))  # the echo
        a._got_state(state(visible=["w1:t2"], view_filter="running"))  # another panel's choice
        assert a.view.filter == "running"
        a._got_state(state(visible=["w1:t2"]))  # an older core: no field, the panel's own stays
        assert a.view.filter == "running"


async def test_an_unechoed_choice_gives_way_to_the_core_after_a_moment(paths, monkeypatch):
    from herdr_panorama import panel as viewer
    monkeypatch.setattr(viewer, "FILTER_ECHO_SECONDS", 0.05)
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(visible=["w1:t2"], view_filter="all"))
        monkeypatch.setattr(a.client, "filter", lambda name: True)
        a._set_filter("needs")  # its echo was lost (a full outbox drops old states)
        await pilot.pause(0.1)
        a._got_state(state(visible=["w1:t2"], view_filter="running"))
        assert a.view.filter == "running"


async def test_after_a_lost_core_the_next_cores_filter_wins_at_once(paths, monkeypatch):
    """A choice still waiting for its echo when the core goes: the next core's state decides
    (a home restart starts at "all"), not this panel's unconfirmed choice."""
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a._got_state(state(visible=["w1:t2"], view_filter="all"))
        await pilot.pause()
        monkeypatch.setattr(a.client, "filter", lambda name: True)
        a._set_filter("needs")
        a._status(False)
        a._status(True)
        a._got_state(state(visible=["w1:t2"], view_filter="all"))
        assert a.view.filter == "all"


# -- 2. navigation among what the filter shows -------------------------------------------

async def test_arrows_in_a_panel_whose_orchestrator_the_filter_hides_step_from_it(paths):
    """Tab 4's panel, filter "needs": its own w1:p4 is hidden. ↑ goes to w1:p3, the shown one
    above it (it went to the top, w1:p1); ↓ goes to the next shown row below it."""
    b = panel(paths, "w1:t4", "w1:p94")
    async with b.run_test(size=(46, 45)) as pilot:
        b.view.filter = "needs"  # chosen here (the navigation alone, without the sync)
        b._got_state(state(visible=["w1:t4"]))
        await pilot.pause()
        assert b.view.filter == "needs" and b.view.orch is None  # no selected row: it is hidden
        assert b._selection() == (None, None, None)  # Enter / g act on nothing hidden
        await pilot.press("up")
        assert b.view.orch == "w1:p3"
        b._got_state(state(visible=["w1:t3"]))  # its switch happened ...
        b._got_state(state(visible=["w1:t4"]))  # ... and back to tab 4
        assert b.view.orch is None
        await pilot.press("down")
        assert b.view.orch == render.NEW_ROW  # nothing shown below w1:p4 but + New agent


async def test_a_filter_change_in_a_panel_parks_a_hidden_shown_orchestrator(paths):
    b = panel(paths, "w1:t4", "w1:p94")
    async with b.run_test(size=(46, 45)) as pilot:
        b._got_state(state(visible=["w1:t4"], view_filter="all"))
        await pilot.pause()
        assert b.view.orch == "w1:p4"
        b._got_state(state(visible=["w1:t4"], view_filter="needs"))  # another tab chose "needs"
        assert b.view.orch is None
        await pilot.press("up")
        assert b.view.orch == "w1:p3"
        b._got_state(state(visible=["w1:t4"], view_filter="all"))  # shown again: still the arrow's row
        assert b.view.orch == "w1:p3"
        b.view.orch = None
        b._apply_filter("all")  # a parked cursor comes back to the orchestrator herdr shows
        assert b.view.orch == "w1:p4"


async def test_arrows_in_the_home_view_step_from_a_row_the_filter_just_hid(fake_herdr):
    """The home panorama: w1:p3 selected under "needs"; a refresh answers its questions, so the
    filter hides it. ↑ goes to w1:p2 (it went to the top)."""
    from herdr_panorama.tui import SwarmApp
    sw = design_swarm(NINE["data"])
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, home_mode=True, cockpit_mode=False)
    async with app.run_test(size=(160, 45)) as pilot:
        await pilot.pause(0.2)
        app._set_filter("needs")
        app._select(0, "w1:p3")
        app._focus(0)
        sw2 = copy.deepcopy(sw)
        render.find_orch(sw2, "w1:p3").inbox = []
        app.apply(sw2)
        assert app.view.orch is None
        await pilot.press("up")
        assert app.view.orch == "w1:p2"


async def test_the_shown_orchestrator_is_selected_again_when_the_filter_shows_it_again(paths):
    """Review of #54 (M1): tab 3's panel under "needs"; w1:p3's questions are answered (hidden,
    no selected row), then a new one arrives: w1:p3 is the selected row again, without waiting
    for a tab switch."""
    b = panel(paths, "w1:t3", "w1:p93")
    sw = design_swarm(NINE["data"])
    async with b.run_test(size=(46, 45)) as pilot:
        b._got_state(state(visible=["w1:t3"], view_filter="needs", sw=sw))
        await pilot.pause()
        assert b.view.orch == "w1:p3"
        sw2 = copy.deepcopy(sw)
        render.find_orch(sw2, "w1:p3").inbox = []
        b._got_state(state(visible=["w1:t3"], view_filter="needs", sw=sw2))
        assert b.view.orch is None
        b._got_state(state(visible=["w1:t3"], view_filter="needs", sw=sw))  # needs the operator again
        assert b.view.orch == "w1:p3"
