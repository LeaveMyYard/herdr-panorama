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
NO_CORE_SECONDS = 600.0  # no core this long: the viewer exits, and herdr closes its pane
NO_CORE_CHECK = 10.0  # how often that is looked at


def crash_marker(pane_id: str):
    """Left by a viewer that died of an error, so the core does not take its vanished pane for
    the operator closing it."""
    from . import home
    return home.state_dir() / f"panel-crash-{pane_id}"


def mark_crash(pane_id: str | None) -> None:
    if not pane_id:
        return
    try:
        crash_marker(pane_id).parent.mkdir(parents=True, exist_ok=True)
        crash_marker(pane_id).write_text(str(time.time()))
    except OSError:
        pass


class PanelClient:
    """The viewer's side of the socket, on a thread. ``on_state(state)``, ``on_status(connected:
    bool)`` and ``on_shutdown()`` (the core says panels go: quit, per-tab off) are called from
    that thread."""

    def __init__(self, sock_path, pane_id: str | None, tab_id: str | None, on_state, on_status,
                 on_shutdown=None):
        self.sock_path = str(sock_path)
        self.pane_id, self.tab_id = pane_id, tab_id
        self.on_state, self.on_status = on_state, on_status
        self.on_shutdown = on_shutdown or (lambda: None)
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
                    elif "shutdown" in msg and welcomed:
                        self._stop.set()
                        self.on_shutdown()
                        return welcomed
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
                         cockpit_mode=True)  # Enter, a click and the arrows switch chats (_dock below)
        self.panes_of: dict[str, str] = {}  # tab -> its panel, from the core's state
        self.my_pane = pane_id if pane_id is not None else os.environ.get("HERDR_PANE_ID")
        self.my_tab = tab_id if tab_id is not None else os.environ.get("HERDR_TAB_ID")
        self.connected = False
        self.visible = True
        self.client = PanelClient(sock_path or protocol.socket_path(), self.my_pane, self.my_tab,
                                  on_state=lambda st: self._to_ui(self._got_state, st),
                                  on_status=lambda on: self._to_ui(self._status, on),
                                  on_shutdown=lambda: self._to_ui(self.exit))
        self.no_core_since: float | None = None
        self._start_client = start_client

    def on_mount(self) -> None:  # no collect, no follow: the core does those
        self.set_interval(self.colour_seconds(), self.animate)
        if not self.connected:
            self.no_core_since = time.monotonic()
        self.set_interval(NO_CORE_CHECK, self._no_core_check)
        if self._start_client:
            self.client.start()

    def _no_core_check(self) -> None:
        """No core for NO_CORE_SECONDS (it crashed, or per-tab went off while it was down): go,
        so no panel is left behind for good; the next core makes them again."""
        if self.no_core_since is not None and time.monotonic() - self.no_core_since >= NO_CORE_SECONDS:
            log.info("no core for %ss: closing", NO_CORE_SECONDS)
            self.exit()

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
        got = state.get("panels")
        self.panes_of = {t: p for t, p in got.items() if isinstance(t, str) and isinstance(p, str)} \
            if isinstance(got, dict) else {}
        self._latest = swarm
        if not self.loaded:
            self.focus_pane = mine
        self.apply(swarm, marks=unseen)
        focused = state.get("focused_pane")
        if isinstance(focused, str):
            self.herdr_focus(focused)

    def _status(self, connected: bool) -> None:
        self.connected = connected
        self.no_core_since = None if connected else (self.no_core_since or time.monotonic())
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

    # -- switching chats: a tab switch, the panel already there --------------------------

    def _fit(self) -> None:  # the core sizes the panels when it places them
        return

    def _dock(self, pane_id: str, keep_focus: bool = False) -> None:
        """Instead of moving a pane: show the target's tab, the keyboard on its panel
        (``keep_focus``) or on the agent. Re-checked against a fresh herdr snapshot."""
        if self.docking or not self.connected:
            return
        self.docking = True
        self.run_worker(lambda: self._switch_worker(pane_id, keep_focus), thread=True, group="dock")

    def _switch_worker(self, pane_id: str, keep_focus: bool) -> None:
        err = None
        try:
            snap = herdr.snapshot() or {}
            agent = next((p for p in snap.get("panes") or [] if isinstance(p, dict) and p.get("pane_id") == pane_id), None)
            if agent is None or not agent.get("tab_id"):
                err = f"{pane_id} is gone"
            else:
                tab = agent["tab_id"]
                panel = self.panes_of.get(tab)
                here = {p.get("pane_id") for p in snap.get("panes") or [] if isinstance(p, dict) and p.get("tab_id") == tab}
                if keep_focus and panel in here:
                    err = herdr.focus_tab(tab) or herdr.focus_pane(panel)
                else:  # into the agent; a tab without a panel yet gets one there (the core)
                    err = herdr.focus_agent(pane_id)
        except Exception as exc:  # noqa: BLE001 - never kill the view
            err = f"{type(exc).__name__}: {exc}"
        self._to_ui(self._docked, err)

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


def request_open(sock_path=None, timeout: float = 8.0) -> str | None:
    """The open key in per-tab mode: ask the core (it decides: this tab's panel, a new one, or
    the home tab). What it did, or None when there is no core of ours to ask."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.settimeout(timeout)
        s.connect(str(sock_path or protocol.socket_path()))
        if protocol.peer_cred(s)[1] != os.getuid():
            return None
        s.sendall(protocol.encode({"hello": {"protocol": protocol.PROTOCOL, "role": "key"}})
                  + protocol.encode({"open": {}}))
        reader = protocol.LineReader()
        while True:
            data = s.recv(65536)
            if not data:
                return None
            for msg in reader.feed(data):
                if "opened" in msg and isinstance(msg["opened"], dict):
                    return str(msg["opened"].get("result") or "failed")
                if "error" in msg:
                    return None
    except (OSError, protocol.TooLong):
        return None
    finally:
        try:
            s.sendall(protocol.encode({"bye": {}}))
        except OSError:
            pass
        s.close()


def run() -> int:
    app = PanelApp()
    try:
        app.run()
    except Exception:
        mark_crash(app.my_pane)
        raise
    if app.return_code:  # Textual caught an error: a crash, not the operator closing the panel
        mark_crash(app.my_pane)
    return app.return_code or 0
