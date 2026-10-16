"""Per-tab panels, first part: the protocol, the core's publisher and its hardening, and the panel
viewer (docs/DESIGN.md "Per-tab panels"; the review of #29: M1, M4, M5, M7, L1, L4)."""

import json
import os
import socket
import stat
import time

import pytest

from herdr_panorama import core, protocol, render
from herdr_panorama.model import Swarm

from .design_dock import FIX, design_swarm

NINE = json.loads((FIX / "design_9.json").read_text())


def wait(cond, timeout=3.0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if cond():
            return True
        time.sleep(0.02)
    return cond()


# -- protocol ------------------------------------------------------------------------------

def test_the_model_round_trips_on_the_wire():
    sw = design_swarm(NINE["data"])
    back = protocol.swarm_from_wire(json.loads(json.dumps(protocol.swarm_to_wire(sw))))
    assert protocol.swarm_to_wire(back) == protocol.swarm_to_wire(sw)


def test_unknown_fields_are_ignored_and_versions_compare_by_major():
    wire = protocol.swarm_to_wire(design_swarm(NINE["data"]))
    wire["orchestrators"][0]["from_a_newer_minor"] = 1
    wire["orchestrators"][0]["threads"][0]["agents"][0]["also_new"] = "x"
    assert protocol.swarm_from_wire(wire).orchestrators[0].pane_id == wire["orchestrators"][0]["pane_id"]
    assert protocol.compatible("1.7") and not protocol.compatible("2.0") and not protocol.compatible(None)


def test_lines_are_capped(monkeypatch):
    monkeypatch.setattr(protocol, "MAX_LINE", 64)
    r = protocol.LineReader()
    assert r.feed(b'{"a":1}\n{"b"') == [{"a": 1}]
    with pytest.raises(protocol.TooLong):
        r.feed(b"x" * 100)
    with pytest.raises(protocol.TooLong):
        protocol.encode({"s": "y" * 100})


def test_a_long_state_dir_falls_back_to_a_short_socket_path(monkeypatch, tmp_path):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / ("x" * 120)))
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    assert len(str(protocol.socket_path())) <= protocol.SUN_PATH_MAX
    assert protocol.socket_path().parent.name.startswith("herdr-panorama-")


# -- the core ----------------------------------------------------------------------------------

@pytest.fixture
def paths():
    """A short directory: Unix socket paths are limited (macOS's temp dirs are long)."""
    import shutil
    import tempfile
    from pathlib import Path
    base = Path(tempfile.mkdtemp(prefix="pc", dir="/tmp"))
    d = base / "s"
    yield d / "core.sock", d / "core.lock"
    shutil.rmtree(base, ignore_errors=True)


def connect(sock):
    c = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    c.connect(str(sock))
    c.settimeout(3)
    return c


def lines(c, n):
    r, out = protocol.LineReader(), []
    while len(out) < n:
        out += r.feed(c.recv(65536))
    return out


def test_one_core_per_session_and_its_socket_is_private(paths):
    sock, lock = paths
    first = core.Core.start(sock, lock)
    assert first is not None
    assert core.Core.start(sock, lock) is None  # the lock is held: no second core
    assert stat.S_IMODE(os.stat(sock).st_mode) == 0o600
    assert stat.S_IMODE(os.stat(sock.parent).st_mode) == 0o700
    first.stop()
    assert not sock.exists()
    again = core.Core.start(sock, lock)  # the lock went with it
    assert again is not None
    again.stop()


def test_a_dead_cores_socket_is_replaced_under_the_lock(paths):
    sock, lock = paths
    sock.parent.mkdir(mode=0o700)
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.bind(str(sock))  # left behind: nobody listens
    s.close()
    c = core.Core.start(sock, lock)
    assert c is not None
    v = connect(sock)
    v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "pane_id": "w1:p9"}}))
    assert "welcome" in lines(v, 1)[0]
    v.close()
    c.stop()


def test_another_users_connection_is_refused(paths, monkeypatch):
    sock, lock = paths
    c = core.Core.start(sock, lock)
    monkeypatch.setattr(protocol, "peer_cred", lambda conn: (1, os.getuid() + 1))
    v = connect(sock)
    try:
        v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL}}))
        assert v.recv(100) == b""  # closed without a word
    except (BrokenPipeError, ConnectionResetError):
        pass  # closed before the hello arrived: refused all the same
    assert not c.connected()
    c.stop()


def test_hello_welcome_state_seen_and_an_old_major_refused(paths):
    sock, lock = paths
    seen = []
    c = core.Core.start(sock, lock, on_seen=seen.extend, verify_pane=lambda p, pid: p == "w1:p9")
    c.publish(protocol.state_message(design_swarm(NINE["data"]), {"a5": "done"}, "w1:p3", ["w1:t3"]))
    v = connect(sock)
    v.sendall(protocol.encode({"hello": {"protocol": "1.3", "role": "panel", "pane_id": "w1:p9", "tab_id": "w1:t3"}}))
    welcome, state = lines(v, 2)
    assert welcome == {"welcome": {"protocol": protocol.PROTOCOL}}
    assert state["state"]["unseen"] == {"a5": "done"} and state["state"]["visible_tabs"] == ["w1:t3"]
    assert wait(lambda: c.connected() and c.connected()[0].pane_id == "w1:p9")  # verified claim
    v.sendall(protocol.encode({"seen": ["a5", 7]}))
    assert wait(lambda: seen == ["a5"])
    old = connect(sock)
    old.sendall(protocol.encode({"hello": {"protocol": "2.0"}}))
    assert "error" in lines(old, 1)[0]
    assert old.recv(100) == b""
    v.close()
    c.stop()


def test_an_unverified_pane_claim_is_not_trusted(paths):
    sock, lock = paths
    c = core.Core.start(sock, lock, verify_pane=lambda p, pid: False)
    v = connect(sock)
    v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "pane_id": "w9:p1"}}))
    lines(v, 1)
    assert wait(lambda: c.connected()) and c.connected()[0].pane_id is None
    v.close()
    c.stop()


def test_a_stuck_viewer_never_stalls_the_core(paths):
    """A viewer that never reads: its outbox stays bounded, publish never blocks."""
    sock, lock = paths
    c = core.Core.start(sock, lock)
    stuck = connect(sock)
    stuck.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL}}))
    assert wait(lambda: c.connected())
    big = protocol.state_message(design_swarm(NINE["data"]), {}, None, None)
    t0 = time.monotonic()
    for _ in range(300):  # far more than the socket buffer holds
        c.publish(big)
    assert time.monotonic() - t0 < 2.0
    assert all(v.outbox.qsize() <= core.OUTBOX for v in c.connected())
    stuck.close()
    c.stop()


def test_messages_before_hello_close_the_connection(paths):
    sock, lock = paths
    seen = []
    c = core.Core.start(sock, lock, on_seen=seen.extend)
    v = connect(sock)
    v.sendall(protocol.encode({"seen": ["a1"]}))
    assert v.recv(100) == b"" and seen == []
    c.stop()


# -- the viewer --------------------------------------------------------------------------------

def test_the_client_queues_seen_while_the_core_is_away(paths):
    """L1: marks made while there is no core are sent after the next welcome."""
    sock, lock = paths
    from herdr_panorama.panel import PanelClient
    states, status = [], []
    cl = PanelClient(sock, "w1:p9", "w1:t3", states.append, status.append).start()
    cl.seen(["a1", "a2"])  # no core yet: queued
    seen = []
    c = core.Core.start(sock, lock, on_seen=seen.extend)
    c.publish(protocol.state_message(Swarm(), {}, None, None))
    assert wait(lambda: status == [True], 5) and wait(lambda: seen == ["a1", "a2"])
    assert wait(lambda: states)
    c.stop()  # the core goes away: the client says so, then reconnects to a new one
    assert wait(lambda: status == [True, False], 5)
    c2 = core.Core.start(sock, lock)
    assert wait(lambda: status == [True, False, True], 8)
    cl.stop()
    c2.stop()


async def test_the_panel_renders_its_tab_exactly_like_the_docked_panel(paths):
    """The viewer's picture is today's docked panel (9e) for its own tab's orchestrator."""
    sock, lock = paths
    from herdr_panorama.panel import PanelApp
    sw = design_swarm(NINE["data"])
    c = core.Core.start(sock, lock)
    c.publish(protocol.state_message(sw, {}, None, ["w1:t3"]))
    app = PanelApp(sock_path=sock, pane_id="w1:p9", tab_id="w1:t3")
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(100):
            await pilot.pause(0.03)
            if app.loaded and app.connected:
                break
        assert app.connected and app.swarm.docked == "w1:p3" and app.view.dock
        got = app.grid.screen_data
        want_sw = protocol.swarm_from_wire(protocol.swarm_to_wire(sw))
        want_sw.docked = "w1:p3"
        v = render.View(orch=app.view.orch, dock=True, frame=app.view.frame, now=app.view.now)
        v.panel, v.unseen = app.view.panel, app.view.unseen
        want = render.render(want_sw, v, 46, 45)
        assert [[(x.ch, x.fg, x.bg) for x in r] for r in got.c] == [[(x.ch, x.fg, x.bg) for x in r] for r in want.c]
        c.stop()  # the core goes away: dimmed, with the operator's notice; actions off
        for _ in range(100):
            await pilot.pause(0.03)
            if not app.connected:
                break
        from herdr_panorama.panel import NOTICE
        assert not app.connected and NOTICE in app.grid.screen_data.text().splitlines()[0]
        await pilot.press("q")


async def test_a_panel_without_a_core_acts_on_nothing(paths, fake_herdr):
    sock, lock = paths
    from herdr_panorama.panel import PanelApp
    app = PanelApp(sock_path=sock, pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app._got_state(protocol.state_message(design_swarm(NINE["data"]), {}, None, None)["state"])
        app.armed_at = 0.0
        await pilot.press("enter")
        await pilot.pause(0.2)
    assert fake_herdr.calls_of("agent", "focus") == []


async def test_a_hidden_tabs_panel_draws_no_frames(paths):
    """M5: the core says which tab is shown; a panel elsewhere stops animating."""
    from herdr_panorama.panel import PanelApp
    app = PanelApp(sock_path=paths[0], pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app._got_state(protocol.state_message(design_swarm(NINE["data"]), {}, None, ["w1:t1"])["state"])
        assert not app.visible
        f = app.view.frame
        for _ in range(10):
            app.animate()
        await pilot.pause()
        assert app.view.frame == f
        app._got_state(protocol.state_message(design_swarm(NINE["data"]), {}, None, None)["state"])
        assert app.visible  # unknown counts as visible


async def test_the_panels_focus_look_follows_the_cores_focused_pane(paths):
    from herdr_panorama.panel import PanelApp
    from herdr_panorama import theme
    app = PanelApp(sock_path=paths[0], pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app._got_state(protocol.state_message(design_swarm(NINE["data"]), {}, "w1:p3", None)["state"])
        await pilot.pause()
        assert not app.pane_focused and app.grid.screen_data.c[30][20].bg == theme.FIXED[1]
        app._got_state(protocol.state_message(design_swarm(NINE["data"]), {}, "w1:p9", None)["state"])
        await pilot.pause()
        assert app.pane_focused and app.grid.screen_data.c[30][20].bg == theme.FIXED[0]


def test_the_home_panorama_publishes_only_in_per_tab_mode(monkeypatch):
    from herdr_panorama import cockpit
    monkeypatch.setattr(cockpit.config, "load", lambda: {})
    assert cockpit.panels_mode() == "moving"
    monkeypatch.setattr(cockpit.config, "load", lambda: {"cockpit_panels": "per-tab"})
    assert cockpit.panels_mode() == "per-tab"




async def test_the_home_panorama_publishes_each_refresh_and_takes_seen_back(paths, fake_herdr):
    """The core's side inside the home panorama: every applied refresh goes out as a full
    state; a viewer's seen clears the mark in the home tracker."""
    sock, lock = paths
    from herdr_panorama.tui import SwarmApp
    sw = design_swarm(NINE["data"])
    app = SwarmApp(source=lambda: sw, live_refresh=False, clock=lambda: 0.0, focus_pane="w1:p3",
                   home_mode=True, cockpit_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        app.core = core.Core.start(sock, lock, on_seen=lambda ids: app._to_ui(app._seen, ids))
        v = connect(sock)
        v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL}}))
        await pilot.pause(0.2)
        assert "welcome" in lines(v, 1)[0]
        app.view.unseen = {"a5": "done"}
        app.apply(sw, marks={"a5": "done"})
        await pilot.pause(0.2)
        state = next(m for m in lines(v, 1) if "state" in m)["state"]
        assert state["unseen"] == {"a5": "done"} and len(state["swarm"]["orchestrators"]) == len(sw.orchestrators)
        cleared = []
        app._seen = lambda ids: cleared.extend(ids)
        v.sendall(protocol.encode({"seen": ["a5"]}))
        for _ in range(50):
            await pilot.pause(0.02)
            if cleared:
                break
        assert cleared == ["a5"]
        v.close()
        app.core.stop()
        app.core = None


# -- the review of #30 ------------------------------------------------------------------------

def test_the_line_reader_is_linear_and_one_read_reports_seen_once(paths):
    """Low: one feed splits once (not once per line), and a read's seen ids go out together."""
    many = b'{"seen":["a1"]}\n' * 200_000  # 3 MB: about 6 s when every line re-copies the rest
    t0 = time.monotonic()
    assert len(protocol.LineReader().feed(many)) == 200_000
    assert time.monotonic() - t0 < 1.5
    sock, lock = paths
    calls = []
    c = core.Core.start(sock, lock, on_seen=calls.append)
    v = connect(sock)
    v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL}})
              + protocol.encode({"seen": ["a1"]}) + protocol.encode({"seen": ["a2", "a1"]}))
    assert wait(lambda: calls)
    time.sleep(0.1)
    assert calls == [["a1", "a2"]]
    v.close()
    c.stop()


@pytest.mark.parametrize("bad", ["symlink", "group-writable", "not-ours"])
def test_an_unsafe_socket_directory_is_refused(paths, bad):
    """Medium: the socket's directory must be a real 0700 directory of ours, or no core."""
    sock, lock = paths
    real = sock.parent.parent / "elsewhere"
    real.mkdir(mode=0o700)
    kw = {}
    if bad == "symlink":
        sock.parent.symlink_to(real)
    elif bad == "group-writable":
        sock.parent.mkdir()
        os.chmod(sock.parent, 0o770)
    else:
        sock.parent.mkdir(mode=0o700)
        kw["uid"] = os.getuid() + 1
    assert core.Core.start(sock, lock, **kw) is None
    assert not sock.exists() and not (real / sock.name).exists()


def test_a_viewer_refuses_a_core_of_another_user(paths, monkeypatch):
    """Medium: the viewer checks who answers before it says hello or believes a state."""
    sock, lock = paths
    sock.parent.mkdir(mode=0o700)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(str(sock))
    srv.listen(4)
    srv.settimeout(3)
    monkeypatch.setattr(protocol, "peer_cred", lambda conn: (1, os.getuid() + 1))
    from herdr_panorama.panel import PanelClient
    status = []
    cl = PanelClient(sock, "w1:p9", "w1:t3", lambda st: None, status.append).start()
    conn, _ = srv.accept()
    conn.settimeout(3)
    assert conn.recv(1000) == b""  # closed without a hello
    assert status == [] and not cl.connected
    cl.stop()
    conn.close()
    srv.close()


def test_a_connection_without_hello_is_closed_at_the_deadline(paths, monkeypatch):
    monkeypatch.setattr(core, "HELLO_SECONDS", 0.3)
    sock, lock = paths
    c = core.Core.start(sock, lock)
    v = connect(sock)
    v.sendall(b'{"hel')  # a start, never finished
    t0 = time.monotonic()
    assert v.recv(100) == b"" and time.monotonic() - t0 < 2
    assert wait(lambda: c.viewers == [])
    c.stop()


def test_idle_connections_are_capped_and_hold_no_threads(paths, monkeypatch):
    """Medium: 100 connections that never say hello: at most MAX_VIEWERS are kept, one thread each."""
    import threading
    monkeypatch.setattr(core, "MAX_VIEWERS", 8)
    sock, lock = paths
    c = core.Core.start(sock, lock)
    before = threading.active_count()
    idle = [connect(sock) for _ in range(100)]
    assert wait(lambda: len(c.viewers) == 8)
    time.sleep(0.2)
    assert len(c.viewers) <= 8 and threading.active_count() - before <= 8
    for s in idle:
        s.close()
    c.stop()


def test_a_viewer_that_never_reads_is_dropped_after_the_send_timeout(paths, monkeypatch):
    """Medium: the writer's sendall gives up, the viewer goes, its threads end."""
    import threading
    monkeypatch.setattr(core, "SEND_SECONDS", 0.3)
    sock, lock = paths
    c = core.Core.start(sock, lock)
    stuck = connect(sock)
    stuck.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
    before = threading.active_count()
    stuck.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL}}))
    assert wait(lambda: c.connected())
    big = protocol.state_message(design_swarm(NINE["data"]), {}, None, None)
    end = time.monotonic() + 5
    while c.connected() and time.monotonic() < end:
        c.publish(big)
        time.sleep(0.01)
    assert not c.connected() and wait(lambda: c.viewers == [])
    assert wait(lambda: threading.active_count() <= before)
    stuck.close()
    c.stop()


def test_a_pane_claim_is_checked_against_the_peers_own_process(paths):
    """Low: verify_pane gets the claimed pane and the connecting process's pid (the panel's
    ownership check: that pane's foreground runs this pid as ``panorama panel``)."""
    sock, lock = paths
    asked = []
    c = core.Core.start(sock, lock, verify_pane=lambda pane, pid: asked.append((pane, pid)) or True)
    v = connect(sock)
    v.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "pane_id": "w1:p9"}}))
    lines(v, 1)
    assert asked == [("w1:p9", os.getpid())]
    v.close()
    c.stop()


def test_a_pane_runs_our_panel_only_with_that_pid_and_argv(monkeypatch):
    from herdr_panorama import herdr
    procs = [{"argv": ["uv", "run", "panorama", "panel"], "pid": 10},
             {"argv": ["/v/bin/python3", "/v/bin/panorama", "panel"], "pid": 11}]
    monkeypatch.setattr(herdr, "call", lambda *a, **k: ({"process_info": {"foreground_processes": procs}}, None))
    assert herdr.runs_panorama("w1:p9", "panel", pid=11) is True
    assert herdr.runs_panorama("w1:p9", "panel", pid=12) is False  # another process's claim
    assert herdr.runs_panorama("w1:p9", "tui", pid=11) is False
    procs[1]["argv"] = ["/usr/bin/python3", "-c", "panorama panel"]
    assert herdr.runs_panorama("w1:p9", "panel", pid=11) is False


def test_the_tmp_fallbacks_parent_must_be_safe_too(paths, monkeypatch):
    """Low (#30 review): the socket directory's parent is checked too: ours, or root's with the
    sticky bit (/tmp); a parent another user can write in (without sticky) is refused."""
    sock, lock = paths
    os.chmod(sock.parent.parent, 0o777)  # world-writable, no sticky bit
    assert core.Core.start(sock, lock) is None
    os.chmod(sock.parent.parent, 0o1777)  # like /tmp, but ours: fine
    c = core.Core.start(sock, lock)
    assert c is not None
    c.stop()
    assert core.private_dir(sock.parent, os.getuid()) is None
