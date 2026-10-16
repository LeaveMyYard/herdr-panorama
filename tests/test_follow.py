# ruff: noqa: F811 - pytest fixtures arrive as parameters named like the imported fixture
"""Cockpit follow: when the operator lands on an orchestrator's tab some other way (tab bar,
prefix+N, a notification, `agent focus`), panorama's own pane re-docks there without moving
the operator's focus; skip cases, failures, debounce and coalescing, the event stream, zoom."""

import json
import os
import socket
import tempfile
import threading
import time

import pytest

from herdr_panorama import cockpit, home
from herdr_panorama import herdr as herdr_cli

from .test_cockpit import add_agent, beside_home, configure, in_tab, only_panorama_moved, put, sockets, state, tab_of
from .test_home import herdr  # noqa: F401 - the fixture


@pytest.fixture(autouse=True)
def on():
    configure("cockpit = true\n")


def land(h, pane):
    """The operator focuses ``pane`` (what herdr reports as the focused pane)."""
    st = state(h)
    st["focus"] = pane
    put(h, st)


def focus_of(h):
    return state(h).get("focus")


def stream(h, monkeypatch):
    """A FocusLog fed like herdr's event stream: the swap's own focus event (its source pane)."""
    fl = cockpit.FocusLog()
    fl.live = True
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        err = real(source, target)
        fl.record({"type": "pane_focused", "pane_id": source, "workspace_id": "w1"})
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    return fl


def no_focus_calls(h):
    calls = h.calls()
    assert not [c for c in calls if c[:2] in (["agent", "focus"], ["tab", "focus"], ["pane", "focus"])]
    assert sockets(h, "pane.focus") == []


def test_follow_docks_beside_the_agent_and_the_operator_stays_put(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    land(herdr, a)
    mine, why = cockpit.follow(h.pane_id, stream(herdr, monkeypatch))
    assert (mine, why) == (h.pane_id, "docked")
    assert in_tab(herdr, ta) == [h.pane_id, a]  # panorama on the agent's left
    assert focus_of(herdr) == a  # exactly where the operator was
    assert herdr.calls("pane", "move")[0][-3:] == ["--no-focus", "--target-pane", a]
    assert herdr.calls("pane", "swap") == [["pane", "swap", "--source-pane", a, "--target-pane", h.pane_id]]
    no_focus_calls(herdr)
    only_panorama_moved(herdr, h.pane_id)


@pytest.mark.parametrize("case", ["shell", "panorama", "other workspace", "already there", "other agent kind",
                                  "nothing focused"])
def test_follow_skips(herdr, case):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    if case == "shell":
        st = state(herdr)
        st["tabs"].append({"tab_id": "w1:t8", "label": "sh", "workspace_id": "w1"})
        st["panes"].append({"pane_id": "w1:p8", "tab_id": "w1:t8", "workspace_id": "w1"})
        put(herdr, st)
        land(herdr, "w1:p8")
    elif case == "panorama":
        land(herdr, h.pane_id)
    elif case == "other workspace":
        far, _ = add_agent(herdr, "elsewhere", workspace="w2")
        land(herdr, far)
    elif case == "already there":
        cockpit.dock(a, h.pane_id)
        land(herdr, a)
    elif case == "other agent kind":
        st = state(herdr)
        next(p for p in st["panes"] if p["pane_id"] == a)["agent"] = "codex"
        put(herdr, st)
        land(herdr, a)
    moves = len(herdr.calls("pane", "move"))
    mine, why = cockpit.follow(h.pane_id)
    assert mine is None and len(herdr.calls("pane", "move")) == moves, why
    assert why == {"shell": "not an orchestrator's pane", "panorama": "the operator is in panorama",
                   "other workspace": "another workspace", "already there": "already there",
                   "other agent kind": "not an orchestrator's pane", "nothing focused": "no focused pane"}[case]


def test_follow_reuses_the_rename_before_leaving_a_shared_home_tab(herdr):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)  # the operator split the home tab and runs Claude there
    a, _ = add_agent(herdr, "search")
    land(herdr, a)
    cockpit.follow(h.pane_id)
    assert next(t["label"] for t in state(herdr)["tabs"] if t["tab_id"] == tab) == "billing"
    calls = herdr.calls()
    rename = max(i for i, c in enumerate(calls) if c[:2] == ["tab", "rename"])
    move = min(i for i, c in enumerate(calls) if c[:2] == ["pane", "move"])
    assert rename < move


def test_a_failed_follow_leaves_everything_as_is(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    land(herdr, a)
    monkeypatch.setenv("FAKE_HERDR_FAIL", "pane move --tab")
    mine, why = cockpit.follow(h.pane_id)
    assert mine is None and "could not move" in why
    assert tab_of(herdr, h.pane_id) == h.tab_id and in_tab(herdr, ta) == [a] and focus_of(herdr) == a
    assert herdr.calls("pane", "swap") == []


def test_if_the_operator_moved_on_mid_dock_the_swap_is_skipped(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, _ = add_agent(herdr, "search")
    land(herdr, a)
    real = cockpit.herdr.move_pane_to_tab

    def move_then_operator_switches(*args, **kw):
        out = real(*args, **kw)
        land(herdr, b)  # the operator pressed prefix+N again while panorama moved
        return out
    monkeypatch.setattr(cockpit.herdr, "move_pane_to_tab", move_then_operator_switches)
    mine, _ = cockpit.follow(h.pane_id)
    assert mine == h.pane_id and herdr.calls("pane", "swap") == []  # a swap would pull them back to a
    assert focus_of(herdr) == b


def test_follow_holds_the_home_lock(herdr, monkeypatch):
    import fcntl
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    land(herdr, a)
    seen = []
    real = cockpit.herdr.move_pane_to_tab

    def probe(*args, **kw):
        fd = os.open(home.state_dir() / f"home-{home.session_key()}.lock", os.O_WRONLY)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            seen.append("free")
            fcntl.flock(fd, fcntl.LOCK_UN)
        except BlockingIOError:
            seen.append("held")
        finally:
            os.close(fd)
        return real(*args, **kw)
    monkeypatch.setattr(cockpit.herdr, "move_pane_to_tab", probe)
    cockpit.follow(h.pane_id)
    assert seen == ["held"]


# -- the view: debounce, coalescing, skip conditions, zoom ---------------------------------

async def app_with(fake_herdr, monkeypatch, follow):
    from herdr_panorama.tui import SwarmApp

    from .demo import NOW, demo_swarm
    monkeypatch.setattr(cockpit, "follow", follow)
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True,
                   cockpit_mode=True)
    app.my_pane = "w1:p9"
    return app


async def settle(pilot, seconds=0.4):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        await pilot.pause(0.05)


async def test_a_burst_of_focus_events_follows_once(fake_herdr, monkeypatch):
    calls = []
    app = await app_with(fake_herdr, monkeypatch, lambda me, fl=None, on_docked=None: (calls.append(me), (None, "already there"))[1])
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(5):  # prefix+1, prefix+2, ... in a row: act on where the operator ends up
            app.focus_event()
            await pilot.pause(0.03)
        await settle(pilot)
        assert calls == ["w1:p9"]


async def test_events_during_a_follow_are_coalesced_into_one_more(fake_herdr, monkeypatch):
    gate, calls = threading.Event(), []

    def slow(me, fl=None, on_docked=None):
        calls.append(me)
        gate.wait(2)
        return None, "already there"
    app = await app_with(fake_herdr, monkeypatch, slow)
    async with app.run_test(size=(46, 45)) as pilot:
        app.focus_event()
        await settle(pilot, 0.3)
        assert app.following and len(calls) == 1
        for _ in range(3):
            app.focus_event()
            await pilot.pause(0.03)
        await settle(pilot, 0.3)
        assert len(calls) == 1  # never two at once
        gate.set()
        await settle(pilot, 0.4)
        assert len(calls) == 2  # one more look, for all three events


@pytest.mark.parametrize("why", ["form", "docking", "follow off", "cockpit off"])
async def test_no_follow_while(fake_herdr, monkeypatch, why):
    calls = []
    app = await app_with(fake_herdr, monkeypatch, lambda me, fl=None, on_docked=None: (calls.append(me), (None, "x"))[1])
    async with app.run_test(size=(46, 45)) as pilot:
        await settle(pilot, 0.2)
        if why == "form":
            await pilot.press("n")
            await pilot.pause()
        elif why == "docking":
            app.docking = True
        elif why == "follow off":
            configure("cockpit = true\ncockpit_follow = false\n")
        else:
            app._cockpit_override = False
        app.focus_event()
        await settle(pilot)
        assert calls == []


async def test_a_follow_updates_the_views_own_pane(fake_herdr, monkeypatch):
    app = await app_with(fake_herdr, monkeypatch, lambda me, fl=None, on_docked=None: ("w2:p5", "docked"))
    async with app.run_test(size=(46, 45)) as pilot:
        app.focus_event()
        await settle(pilot)
        assert app.my_pane == "w2:p5" and app.collector.self_pane == "w2:p5"


async def test_z_zooms_panoramas_own_pane(fake_herdr, monkeypatch):
    app = await app_with(fake_herdr, monkeypatch, lambda me, fl=None, on_docked=None: (None, "x"))
    async with app.run_test(size=(46, 45)) as pilot:
        await settle(pilot, 0.2)
        await pilot.press("z")
        await pilot.pause()
    assert fake_herdr.calls_of("pane", "zoom") == [["pane", "zoom", "w1:p9", "--toggle"]]


def test_the_docked_panel_and_the_graph_show_the_zoom_key():
    from herdr_panorama import render

    from .design_dock import design_swarm
    v = render.View(orch="w1:p3", dock=True, zoom_hint=True)
    scr = render.render(design_swarm(), v, 46, 45)
    assert "z graph" in scr.text().splitlines()[-2]
    v = render.View(orch="w1:p3", zoom_hint=True)
    assert "z panel" in render.render(design_swarm(), v, 160, 45).text().splitlines()[-1]


# -- the event stream ----------------------------------------------------------------------

def test_event_stream_yields_focus_events_and_reconnects(monkeypatch):
    d = tempfile.mkdtemp(prefix="hp", dir="/tmp")  # macOS caps Unix socket paths at 104 bytes
    path = os.path.join(d, "s")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(path)
    srv.listen(4)
    subs = []

    def serve():
        for n in range(2):
            conn, _ = srv.accept()
            with conn:
                subs.append(json.loads(conn.makefile().readline()))
                conn.sendall(b'{"id":"panorama:events","result":{"type":"subscription_started"}}\n')
                conn.sendall(b'not json\n')
                ev = {"data": {"type": "tab_focused", "tab_id": f"w1:t{n}", "workspace_id": "w1"}, "event": "tab_focused"}
                conn.sendall((json.dumps(ev) + "\n").encode())
    threading.Thread(target=serve, daemon=True).start()
    monkeypatch.setenv("HERDR_SOCKET_PATH", path)
    got = []
    for data in herdr_cli.events(["tab.focused"], stop=lambda: len(got) >= 2, timeout=0.2):
        if data.get("type") == "tab_focused":
            got.append(data["tab_id"])
    srv.close()
    os.unlink(path)
    os.rmdir(d)
    assert got == ["w1:t0", "w1:t1"]  # the second after a reconnect
    assert subs[0]["method"] == "events.subscribe" and subs[0]["params"] == {"subscriptions": [{"type": "tab.focused"}]}


# -- review of #12 --------------------------------------------------------------------------

def test_r12_m1_a_switch_in_the_swap_window_gets_focus_sent_back(herdr, monkeypatch):
    """The operator pressed prefix+N between the guard's look and the swap: herdr's swap
    pulls them back to the agent; the event stream shows where they went, focus goes there."""
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, _ = add_agent(herdr, "search")
    land(herdr, a)
    fl = cockpit.FocusLog()
    fl.live = True
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        land(herdr, b)  # the operator's switch reaches herdr first...
        fl.record({"type": "pane_focused", "pane_id": b, "workspace_id": "w1"})
        err = real(source, target)  # ...then the swap focuses its source (the agent)
        fl.record({"type": "pane_focused", "pane_id": source, "workspace_id": "w1"})
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    cockpit.follow(h.pane_id, fl)
    assert sockets(herdr, "pane.focus") == [{"pane_id": b}]
    assert focus_of(herdr) == b  # where they went, not back on billing


def test_r12_m1_a_tab_switch_in_the_window_goes_to_that_tabs_pane(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, tb = add_agent(herdr, "search")
    land(herdr, a)
    fl = cockpit.FocusLog()
    fl.live = True
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        fl.record({"type": "tab_focused", "tab_id": tb, "workspace_id": "w1"})
        err = real(source, target)
        fl.record({"type": "pane_focused", "pane_id": source, "workspace_id": "w1"})
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    cockpit.follow(h.pane_id, fl)
    assert herdr.calls("tab", "focus")[-1] == ["tab", "focus", tb]


def test_r12_m1_without_the_event_stream_follow_does_not_swap(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    land(herdr, a)
    mine, _ = cockpit.follow(h.pane_id, None)
    assert mine == h.pane_id and herdr.calls("pane", "swap") == []  # panorama stays on the right
    assert in_tab(herdr, ta) == [a, h.pane_id] and focus_of(herdr) == a


def test_r12_m1_no_switch_means_no_refocus(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    land(herdr, a)
    cockpit.follow(h.pane_id, stream(herdr, monkeypatch))
    assert sockets(herdr, "pane.focus") == []


def test_r12_m2_follow_tabs_any_by_default_single_on_request(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing", extra_panes=1)  # a work tab: Claude beside an editor
    land(herdr, a)
    configure("cockpit = true\ncockpit_follow_tabs = \"single\"\n")
    assert cockpit.follow(h.pane_id, stream(herdr, monkeypatch)) == (None, "a tab with other panes")
    configure("cockpit = true\n")  # the default: "any" (the operator's decision)
    assert cockpit.follow(h.pane_id, stream(herdr, monkeypatch))[0] == h.pane_id
    assert tab_of(herdr, h.pane_id) == ta
    st = state(herdr)  # an explicit Enter docks anywhere with "single" too
    configure("cockpit = true\ncockpit_follow_tabs = \"single\"\n")
    b, tb = add_agent(herdr, "search", extra_panes=1)
    assert cockpit.dock(b, h.pane_id)[0] == h.pane_id and tab_of(herdr, h.pane_id) == tb
    del st


def test_r12_l2_no_follow_while_panorama_is_zoomed(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    land(herdr, a)
    monkeypatch.setattr(cockpit.herdr, "pane_layout", lambda pane: {"zoomed": True, "panes": []})
    assert cockpit.follow(h.pane_id, stream(herdr, monkeypatch)) == (None, "panorama is zoomed")
    assert herdr.calls("pane", "move") == []


def test_r12_l1_backoff_grows_until_data_arrives(monkeypatch):
    d = tempfile.mkdtemp(prefix="hp", dir="/tmp")
    path = os.path.join(d, "s")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(path)
    srv.listen(8)

    def serve():
        while True:
            try:
                conn, _ = srv.accept()
            except OSError:
                return
            conn.close()  # accepts, then closes before any ack (an older herdr, a shutdown)
    threading.Thread(target=serve, daemon=True).start()
    monkeypatch.setenv("HERDR_SOCKET_PATH", path)
    sleeps = []
    monkeypatch.setattr(time, "sleep", lambda s: sleeps.append(s))
    for _ in herdr_cli.events(["tab.focused"], stop=lambda: len(sleeps) >= 7, timeout=0.2):
        pass
    srv.close()
    os.unlink(path)
    os.rmdir(d)
    assert sleeps[:7] == [0.5, 1.0, 2.0, 4.0, 8.0, 10.0, 10.0]  # grows, capped; never stuck at 0.5


# -- final review of #12 ---------------------------------------------------------------------

def live_stream(h, monkeypatch):
    fl = stream(h, monkeypatch)
    fl.live = True
    return fl


def test_r12f_m1_the_operator_back_in_panorama_before_the_move_stops_it(herdr, monkeypatch):
    """They flipped to an agent's tab and straight back to panorama: the decision snapshot saw
    the agent, but by the move they are in panorama again; nothing moves or is renamed."""
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)  # a shared home tab: the rename would come first
    a, ta = add_agent(herdr, "search")
    land(herdr, a)
    real = cockpit.herdr.pane_layout

    def layout_then_back(pane):
        land(herdr, h.pane_id)  # the second switch lands during the decision's checks
        return real(pane)
    monkeypatch.setattr(cockpit.herdr, "pane_layout", layout_then_back)
    mine, why = cockpit.follow(h.pane_id, live_stream(herdr, monkeypatch))
    assert mine is None and why == "the operator moved on"
    assert herdr.calls("pane", "move") == [] and herdr.calls("tab", "rename")[-1][3] == home.HOME_LABEL
    assert tab_of(herdr, h.pane_id) == tab and focus_of(herdr) == h.pane_id


def test_r12f_l1_a_stream_that_is_not_live_means_no_swap(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    land(herdr, a)
    fl = stream(herdr, monkeypatch)
    fl.live = False  # disconnected (backing off): a switch in the window could go unseen
    mine, _ = cockpit.follow(h.pane_id, fl)
    assert mine == h.pane_id and herdr.calls("pane", "swap") == []


def test_r12f_l1_the_watcher_tracks_liveness(monkeypatch):
    from herdr_panorama.tui import SwarmApp
    seq = [{"type": "_connected"}, {"type": "tab_focused", "tab_id": "w1:t2"}, {"type": "_disconnected"},
           {"type": "_connected"}]
    seen = []

    def events(types, stop, timeout=1.0):
        for d in seq:
            yield d
            seen.append(app.focus_log.live)
    monkeypatch.setattr("herdr_panorama.tui.herdr.events", events)
    app = SwarmApp(source=lambda: None, live_refresh=False, home_mode=True, cockpit_mode=True)
    monkeypatch.setattr(app, "call_from_thread", lambda fn, *a: None)
    app._watch_focus()
    assert seen == [True, True, False, True]
    assert [d.get("tab_id") for d in app.focus_log.since(0)] == ["w1:t2"]  # markers are not focus events


def test_r12f_l1_events_marks_connect_and_disconnect(monkeypatch):
    d = tempfile.mkdtemp(prefix="hp", dir="/tmp")
    path = os.path.join(d, "s")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(path)
    srv.listen(4)

    def serve():
        conn, _ = srv.accept()
        with conn:
            conn.makefile().readline()
            conn.sendall(b'{"id":"panorama:events","result":{"type":"subscription_started"}}\n')
    threading.Thread(target=serve, daemon=True).start()
    monkeypatch.setenv("HERDR_SOCKET_PATH", path)
    monkeypatch.setattr(time, "sleep", lambda s: None)
    got = []
    for data in herdr_cli.events(["tab.focused"], stop=lambda: len(got) >= 2, timeout=0.2):
        got.append(data["type"])
    srv.close()
    os.unlink(path)
    os.rmdir(d)
    assert got == ["_connected", "_disconnected"]


def test_r12f_l2_a_switch_reported_late_still_gets_focus_sent_back(herdr, monkeypatch):
    """The operator's focus event reaches the stream only after the swap call returned (the
    normal case has no swap event at all: the agent was already focused). The bounded wait
    after the swap must still see it."""
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    b, _ = add_agent(herdr, "search")
    land(herdr, a)
    fl = cockpit.FocusLog()
    fl.live = True
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        land(herdr, b)
        err = real(source, target)
        threading.Timer(0.2, fl.record, args=({"type": "pane_focused", "pane_id": b},)).start()
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    cockpit.follow(h.pane_id, fl)
    assert sockets(herdr, "pane.focus") == [{"pane_id": b}]


def test_r12f_l3_a_tab_only_switch_refocuses_with_tab_focus(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    b, tb = add_agent(herdr, "search", extra_panes=1)
    land(herdr, a)
    fl = cockpit.FocusLog()
    fl.live = True
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        fl.record({"type": "tab_focused", "tab_id": tb, "workspace_id": "w1"})
        return real(source, target)
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    cockpit.follow(h.pane_id, fl)
    assert herdr.calls("tab", "focus")[-1] == ["tab", "focus", tb]  # herdr restores that tab's active pane
    assert sockets(herdr, "pane.focus") == []


def test_r12f_refocus_rereads_once_for_a_later_switch(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    b, _ = add_agent(herdr, "search")
    c, _ = add_agent(herdr, "docs")
    land(herdr, a)
    fl = cockpit.FocusLog()
    fl.live = True
    real_swap, real_focus = cockpit.herdr.swap_panes, cockpit.herdr.focus_pane

    def swap(source, target):
        fl.record({"type": "pane_focused", "pane_id": b})
        return real_swap(source, target)

    def focus(pane):
        err = real_focus(pane)
        if pane == b:
            fl.record({"type": "pane_focused", "pane_id": c})  # they kept going while we refocused
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    monkeypatch.setattr(cockpit.herdr, "focus_pane", focus)
    cockpit.follow(h.pane_id, fl)
    assert sockets(herdr, "pane.focus") == [{"pane_id": b}, {"pane_id": c}]


def test_r12f_l4_only_the_home_instance_follows(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    st["tabs"].append({"tab_id": "w1:t9", "label": "x", "workspace_id": "w1"})
    st["panes"].append({"pane_id": "w1:p9", "tab_id": "w1:t9", "workspace_id": "w1", "label": home.HOME_LABEL,
                        "argv": ["uv", "run", "panorama", "tui"]})
    put(herdr, st)
    a, _ = add_agent(herdr, "billing")
    land(herdr, a)
    assert cockpit.follow("w1:p9", live_stream(herdr, monkeypatch)) == (None, "another panorama instance")
    assert herdr.calls("pane", "move") == [] and home._load()["pane_id"] == h.pane_id


def test_r12f_nit_quiet_reasons_have_one_source():
    from herdr_panorama import tui
    assert tui.QUIET_FOLLOW is cockpit.QUIET


# -- review of #16 ----------------------------------------------------------------------------

def test_r16_the_docked_display_is_updated_before_the_settle_wait(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    land(herdr, a)
    order = []
    fl = stream(herdr, monkeypatch)
    real_wait = fl.wait_for
    monkeypatch.setattr(fl, "wait_for", lambda *args, **kw: (order.append("wait"), real_wait(*args, **kw))[1])
    cockpit.follow(h.pane_id, fl, on_docked=lambda agent: order.append(("docked", agent)))
    assert order == [("docked", a), "wait"]


def test_r16_the_settle_wait_keeps_its_margin():
    assert cockpit.SYNC_SECONDS >= 0.3  # herdr delivers focus events up to ~100 ms late (measured)


def test_r16_no_module_wide_last_docked():
    assert not hasattr(cockpit, "last_docked")
