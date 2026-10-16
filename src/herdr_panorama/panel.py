"""A per-tab panel viewer (``panorama panel``, docs/DESIGN.md "Per-tab panels"): the docked 9e
panel of its own tab, rendered from the core's published state (core.py, protocol.py). It
collects nothing and follows nothing; its cursor, hover, arming, focus look and animation
are its own.

Without a core it keeps its last picture, dimmed, with the operator's notice
"panorama is restarting…" (Q139), disables everything that acts on that state, and
reconnects. ``seen`` marks made meanwhile are queued and sent after the next welcome.
"""

from __future__ import annotations

import logging
import os
import socket
import threading
import time

from . import herdr, protocol, render, theme
from .tui import SwarmApp

log = logging.getLogger("herdr_panorama.panel")

NOTICE = "panorama is restarting…"  # the operator's wording (Q139)
DIM = 0.5  # how far a disconnected panel's colours go toward the background
BACKOFF = (0.25, 0.5, 1.0, 2.0, 5.0)  # reconnect waits, the last one repeats


class PanelClient:
    """The viewer's side of the socket, on a thread. ``on_state(state)`` and
    ``on_status(connected: bool)`` are called from that thread."""

    def __init__(self, sock_path, pane_id: str | None, tab_id: str | None, on_state, on_status):
        self.sock_path = str(sock_path)
        self.pane_id, self.tab_id = pane_id, tab_id
        self.on_state, self.on_status = on_state, on_status
        self.connected = False
        self._sock: socket.socket | None = None
        self._pending_seen: list[str] = []
        self._mu = threading.Lock()
        self._stop = threading.Event()
        self.refused: str | None = None  # an incompatible core's message

    def start(self) -> PanelClient:
        threading.Thread(target=self._run, daemon=True, name="panel-client").start()
        return self

    def stop(self) -> None:
        self._stop.set()
        with self._mu:
            s = self._sock
        if s is not None:
            try:
                s.sendall(protocol.encode({"bye": {}}))
            except OSError:
                pass
            try:
                s.close()
            except OSError:
                pass

    def seen(self, ids: list[str]) -> None:
        """Report seen agents; queued while there is no core, sent again after the welcome."""
        with self._mu:
            s = self._sock if self.connected else None
            if s is None:
                self._pending_seen += [i for i in ids if i not in self._pending_seen]
                return
        try:
            s.sendall(protocol.encode({"seen": list(ids)}))
        except OSError:
            with self._mu:
                self._pending_seen += [i for i in ids if i not in self._pending_seen]

    def _set_connected(self, on: bool) -> None:
        if on != self.connected:
            self.connected = on
            self.on_status(on)

    def _run(self) -> None:
        tries = 0
        while not self._stop.is_set():
            if self._session():
                tries = 0
            self._set_connected(False)
            if self._stop.wait(BACKOFF[min(tries, len(BACKOFF) - 1)]):
                return
            tries += 1

    def _session(self) -> bool:
        """One connection, until it ends. True if it got as far as the welcome."""
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            s.connect(self.sock_path)
        except OSError:
            s.close()
            return False
        if protocol.peer_cred(s)[1] != os.getuid():  # not our core: as good as no core
            log.warning("refused %s: the socket is not served by this user", self.sock_path)
            s.close()
            return False
        with self._mu:
            self._sock = s
        welcomed = False
        reader = protocol.LineReader()
        try:
            s.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "role": "panel",
                                                 "pane_id": self.pane_id, "tab_id": self.tab_id}}))
            while not self._stop.is_set():
                data = s.recv(65536)
                if not data:
                    break
                for msg in reader.feed(data):
                    if "welcome" in msg:
                        welcomed = True
                        self.refused = None
                        with self._mu:
                            pending, self._pending_seen = self._pending_seen, []
                        if pending:
                            s.sendall(protocol.encode({"seen": pending}))
                        self._set_connected(True)
                    elif "error" in msg:
                        err = msg["error"] if isinstance(msg["error"], dict) else {}
                        self.refused = str(err.get("message") or "refused")
                    elif "state" in msg and welcomed and isinstance(msg["state"], dict):
                        self.on_state(msg["state"])
        except (OSError, protocol.TooLong) as exc:
            log.info("core connection ended: %s", exc)
        finally:
            with self._mu:
                self._sock = None
            s.close()
        return welcomed


class PanelApp(SwarmApp):
    """The 9e docked panel of this tab, from the core's state."""

    can_zoom = False  # the full graph is the home tab's (the core); no z here

    def action_zoom(self) -> None:
        return

    def __init__(self, sock_path=None, pane_id: str | None = None, tab_id: str | None = None,
                 clock=time.time, start_client: bool = True):
        self._latest = None
        super().__init__(source=lambda: self._latest, clock=clock, live_refresh=False, home_mode=True,
                         cockpit_mode=False)
        self.my_pane = pane_id if pane_id is not None else os.environ.get("HERDR_PANE_ID")
        self.my_tab = tab_id if tab_id is not None else os.environ.get("HERDR_TAB_ID")
        self.connected = False
        self.visible = True
        self.client = PanelClient(sock_path or protocol.socket_path(), self.my_pane, self.my_tab,
                                  on_state=lambda st: self._to_ui(self._got_state, st),
                                  on_status=lambda on: self._to_ui(self._status, on))
        self._start_client = start_client

    def on_mount(self) -> None:  # no collect, no follow: the core does those
        self.set_interval(self.colour_seconds(), self.animate)
        if self._start_client:
            self.client.start()

    def on_unmount(self) -> None:
        self._stopping = True
        self.client.stop()

    def refresh_model(self) -> None:  # the core pushes state; nothing to pull
        return

    # -- from the core ----------------------------------------------------------------

    def _got_state(self, state: dict) -> None:
        swarm = protocol.swarm_from_wire(state.get("swarm") or {})
        mine = next((o.pane_id for o in swarm.orchestrators if o.tab_id and o.tab_id == self.my_tab), None)
        swarm.docked = mine  # this tab's orchestrator: the panel's docked entry
        vis = state.get("visible_tabs")
        self.visible = True if not isinstance(vis, list) or not self.my_tab else self.my_tab in vis
        unseen = state.get("unseen") if isinstance(state.get("unseen"), dict) else {}
        self._latest = swarm
        if not self.loaded:
            self.focus_pane = mine
        self.apply(swarm, marks=unseen)
        focused = state.get("focused_pane")
        if isinstance(focused, str):
            self.herdr_focus(focused)

    def _status(self, connected: bool) -> None:
        self.connected = connected
        self.repaint()

    # -- what the panel does ----------------------------------------------------------

    def animate(self) -> None:
        if self.visible:  # a hidden tab's panel draws no frames
            super().animate()

    def _seen(self, ids) -> None:
        ids = list(ids)
        for i in ids:
            self.view.unseen.pop(i, None)
        self.client.seen(ids)

    def _usable(self) -> bool:
        if not self.connected:
            self.notify(NOTICE, timeout=3)
        return self.connected

    def action_goto(self, switch: bool = False) -> None:
        """Enter / g: into the agent, after a fresh look at herdr (the core's state may be old)."""
        if not self._usable() or not self.armed():
            return
        o, _, _ = self._selection()
        if o is None or o.machine:
            return super().action_goto(switch)  # remote: its hint; parked: nothing
        snap = herdr.snapshot() or {}
        if not any(p.get("pane_id") == o.pane_id for p in snap.get("panes") or [] if isinstance(p, dict)):
            self.notify(f"{o.tab_label} is gone", timeout=3)
            return
        super().action_goto(switch)

    def action_answer(self) -> None:
        if self._usable():
            super().action_answer()

    def action_inbox(self) -> None:
        if self._usable():
            super().action_inbox()

    def action_new_agent(self) -> None:
        if self._usable():
            super().action_new_agent()

    def decorate(self, scr):
        """Without a core: the last picture, dimmed, with the notice on the first line."""
        if self.connected or not self.loaded:
            if not self.loaded and not self.connected:
                scr.put(1, 0, NOTICE, render.DIM)
            return scr
        for row in scr.c:
            for c in row:
                if type(c.fg) is str and c.fg.startswith("#"):
                    c.fg = theme._mix(c.fg, render.BG, DIM)
        scr.put(0, 0, " " * scr.w, render.FG, render.BG)
        scr.put(1, 0, NOTICE, render.FG)
        return scr


def run() -> int:
    app = PanelApp()
    app.run()
    return 0
