
"""herdr's focus event stream, which the per-tab panels' manager reads (its restore guard and
its passes after a focus change), and the zoom key. The moving cockpit's follow, which this
file tested before, is gone (Q187)."""

import json
import os
import socket
import tempfile
import threading
import time

import pytest

from herdr_panorama import cockpit
from herdr_panorama import herdr as herdr_cli

from .test_cockpit import (
    configure,
)
from .test_home import herdr  # noqa: F401 - the fixture


@pytest.fixture(autouse=True)
def on():
    configure("cockpit = true\n")


# -- the view: debounce, coalescing, skip conditions, zoom ---------------------------------

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


# -- review of #16 ----------------------------------------------------------------------------

def test_r16_the_settle_wait_keeps_its_margin():
    assert cockpit.SYNC_SECONDS >= 0.3  # herdr delivers focus events up to ~100 ms late (measured)


