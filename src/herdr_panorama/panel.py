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
import traceback

from . import herdr, protocol, render, theme
from .tui import ARM_SECONDS, SwarmApp

log = logging.getLogger("herdr_panorama.panel")

NOTICE = "panorama is restarting…"  # the operator's wording (Q139)
DIM = 0.5  # how far a disconnected panel's colours go toward the background
BACKOFF = (0.25, 0.5, 1.0, 2.0, 5.0)  # reconnect waits, the last one repeats
NO_CORE_SECONDS = 600.0  # no core this long: the viewer exits, and herdr closes its pane
NO_CORE_CHECK = 10.0  # how often that is looked at
PREPAINT_WAIT = 0.15  # a switch waits at most this long for the core to echo its focus announcement
PREPAINT_SECONDS = 0.02  # then this long for the target panel to paint (a frame of herdr is ~20 ms)
FILTER_ECHO_SECONDS = 1.0  # a filter chosen here wins over the core's older word for this long


def crash_marker(pane_id: str):
    """Left by a viewer that died of an error, so the core does not take its vanished pane for
    the operator closing it."""
    from . import home
    return home.state_dir() / f"panel-crash-{pane_id}"


CRASH_LOG_BYTES = 16384  # the crash log keeps this many bytes of the traceback's end (UTF-8)


def crash_log(pane_id: str):
    """The traceback of the last error that killed this pane's viewer, next to its marker (the
    marker is used up by the core; the log stays for the next report)."""
    return crash_marker(pane_id).with_name(f"panel-crash-{pane_id}.log")


def mark_crash(pane_id: str | None, error: BaseException | None = None) -> None:
    """Never raises: it runs while a crash is already on its way out (review of #47)."""
    if not pane_id:
        return
    try:
        crash_marker(pane_id).parent.mkdir(parents=True, exist_ok=True)
        crash_marker(pane_id).write_text(str(time.time()))
    except OSError:
        return
    if error is None:
        return
    try:
        text = "".join(traceback.format_exception(error))
        data = text.encode("utf-8", errors="backslashreplace")[-CRASH_LOG_BYTES:]
        data = data.decode("utf-8", errors="ignore").encode()  # no half character at the cut
        stamp = time.strftime("%Y-%m-%d %H:%M:%S %z").encode()
        crash_log(pane_id).write_bytes(stamp + b"\n" + data)
    except Exception:  # noqa: BLE001, S110 - the log is evidence only; the marker is what counts
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

    def focusing(self, pane_id: str) -> bool:
        """Announce a switch's focus to the core (it publishes it before the tab is shown).
        False without a core."""
        with self._mu:
            s = self._sock if self.connected else None
        if s is None:
            return False
        try:
            s.sendall(protocol.encode({"focusing": {"pane_id": pane_id}}))
            return True
        except OSError:
            return False

    def filter(self, name: str) -> bool:
        """Tell the core the filter chosen here (1.3), so every view shows it. False without a
        core: the choice stays this panel's until the next core's state."""
        with self._mu:
            s = self._sock if self.connected else None
        if s is None:
            return False
        try:
            s.sendall(protocol.encode({"filter": {"name": name}}))
            return True
        except OSError:
            return False

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
        self.view.cursor_look = False  # the selected row is the shown orchestrator; hover is the mouse's
        self._active: str | None = None  # the orchestrator herdr shows, as last applied to the cursor
        self._awaiting: str | None = None  # a switch's announced focus, until the core echoes it
        self._echo = threading.Event()
        self._filter_sent: tuple[str, float] | None = None  # a filter chosen here, until the core echoes it
        self.my_pane = pane_id if pane_id is not None else os.environ.get("HERDR_PANE_ID")
        self.my_tab = tab_id if tab_id is not None else os.environ.get("HERDR_TAB_ID")
        self.connected = False
        self.tab_shown = True  # this tab is the one herdr shows (not ``visible``: Textual's)
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
        self.tab_shown = True if not isinstance(vis, list) or not self.my_tab else self.my_tab in vis
        unseen = state.get("unseen") if isinstance(state.get("unseen"), dict) else {}
        got = state.get("panels")
        self.panes_of = {t: p for t, p in got.items() if isinstance(t, str) and isinstance(p, str)} \
            if isinstance(got, dict) else {}
        self._latest = swarm
        focused = state.get("focused_pane")
        active = self._herdr_shows(swarm, vis, focused)
        if not self.loaded:
            self.focus_pane = active or mine
        before = self.selected_card() if self.loaded else None
        self.apply(swarm, marks=unseen)
        self._core_filter(state.get("filter"))
        if active and active != self._active:
            # the operator's report: one selected row in every tab's panel, the orchestrator
            # herdr shows; a cursor moved by an arrow (its switch still pending) is left alone
            # until the shown one changes
            self._active = active
            if active not in render.nav(swarm, self.view)[0]:
                # the filter hides it: no row is selected, and the arrows resume from it (_resume)
                self.view.orch, self.view.focus, self._parked_orch = None, 0, active
                self.repaint()
            elif self.view.orch != active:
                self.view.orch, self.view.focus = active, 0
                self._default_children()
                self.repaint()
        elif self._back_to_shown():  # a refresh made the filter show it again (review of #54)
            self.repaint()
        if before is not None and self.selected_card() != before:
            # what a key acts on changed with this state, the cursor moved here included (apply
            # only compares its own part): the keys wait ARM_SECONDS, as for any refresh (#75)
            self.armed_at = max(self.armed_at, self.monotonic() + ARM_SECONDS)
        if isinstance(focused, str):
            self.herdr_focus(focused)
            if focused == self._awaiting:
                self._echo.set()

    def _core_filter(self, name) -> None:
        """The filter every view shows (1.3; an older core sends none: this panel's own stays).
        A filter chosen here is kept against the core's older word until the core echoes it
        or FILTER_ECHO_SECONDS pass."""
        if name not in render.FILTERS:
            return
        if self._filter_sent is not None:
            sent, at = self._filter_sent
            if name == sent or time.monotonic() - at >= FILTER_ECHO_SECONDS:
                self._filter_sent = None
            else:
                return
        if name != self.view.filter:
            self._apply_filter(name)

    def _set_filter(self, name: str) -> None:
        """f or a click on a filter: shown here at once, and sent to the core for the others."""
        self._apply_filter(name)
        self._filter_sent = (name, time.monotonic()) if self.client.filter(name) else None

    def _apply_filter(self, name: str) -> None:
        """The panel's selected row is the orchestrator herdr shows: kept when the filter shows
        it, none when it hides it (the arrows then resume from it); a cursor an arrow moved
        stays while it is shown."""
        v = self.view
        self._filter_to(name)
        if v.orch is None and not self._back_to_shown():
            v.focus = 0
            if self._active:
                self._parked_orch = self._active  # the arrows resume from it (_resume)
        self.repaint()

    def _back_to_shown(self) -> bool:
        """No selected row while the filter shows the orchestrator herdr shows: it is the
        selected row again. Whether it changed anything."""
        v = self.view
        if v.orch is not None or not self._active or self._active not in render.nav(self.swarm, v)[0]:
            return False
        v.orch, v.focus = self._active, 0
        self._default_children()
        return True

    @staticmethod
    def _herdr_shows(swarm, visible, focused) -> str | None:
        """The local orchestrator in the tab herdr shows, else the one whose pane has the focus.
        (Not ``_shown``: that is SwarmApp's own, which every click calls.)"""
        local = [o for o in swarm.orchestrators if not o.machine]
        if isinstance(visible, list):
            hit = next((o.pane_id for o in local if o.tab_id and o.tab_id in visible), None)
            if hit:
                return hit
        return next((o.pane_id for o in local if focused and o.pane_id == focused), None)

    def herdr_focus(self, pane_id: str) -> None:
        """While this panel's own switch runs it keeps its look (it is still on screen until the
        tab changes); the focus is judged again when the switch ends (_docked -> _refocus)."""
        if self.docking and pane_id and pane_id != self.my_pane:
            self.herdr_focused_pane = pane_id
            return
        super().herdr_focus(pane_id)

    def _status(self, connected: bool) -> None:
        self.connected = connected
        if not connected:
            self._filter_sent = None  # the next core's state decides
        self.no_core_since = None if connected else (self.no_core_since or time.monotonic())
        self.repaint()

    # -- what the panel does ----------------------------------------------------------

    def animate(self) -> None:
        if self.tab_shown:  # a hidden tab's panel draws no frames; shown again, the state's
            # repaint (apply) draws the clock's frame at once, not where it paused
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
                    announced = self._prepaint(panel)
                    err = herdr.focus_tab(tab) or herdr.focus_pane(panel)
                    if err and announced:
                        # nothing moved: take the announcement back, so the core stops naming the
                        # target and this panel, still the focused one, keeps its look (review)
                        self.herdr_focused_pane = self.my_pane
                        self.client.focusing(self.my_pane)
                else:  # into the agent; a tab without a panel yet gets one there (the core)
                    err = herdr.focus_agent(pane_id)
        except Exception as exc:  # noqa: BLE001 - never kill the view
            err = f"{type(exc).__name__}: {exc}"
        self._to_ui(self._docked, err)

    def _prepaint(self, panel: str) -> bool:
        """Before showing the target's tab: the core announces the focus, so the target panel
        paints its focused look while hidden and herdr shows it with it on the first frame.
        Whether the announcement went out (a failed switch takes it back)."""
        self._echo.clear()
        self._awaiting = panel
        try:
            sent = self.client.focusing(panel)
            if sent and self._echo.wait(PREPAINT_WAIT):
                time.sleep(PREPAINT_SECONDS)
            return sent
        finally:
            self._awaiting = None

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
    except Exception as exc:
        mark_crash(app.my_pane, exc)
        raise
    if app.return_code:  # Textual caught an error: a crash, not the operator closing the panel
        # Textual keeps the error it handled in the private ``_exception`` (8.2.8; run_test
        # re-raises it); without it the marker is still written, only the log is missing
        mark_crash(app.my_pane, getattr(app, "_exception", None))
    return app.return_code or 0
