"""The swarm view: a Textual app painting ``render.render`` as one cell-grid widget.

Safety rules (from herdr-inbox's reviews, see docs/DESIGN.md):

- the view never types into a pane; ``g`` focuses a pane, ``a``/``i`` open the
  herdr-inbox popup (after this popup has closed);
- cursors are identities (pane id, thread id, agent id), re-resolved after
  every refresh; a vanished target parks the cursor (keys act on nothing);
- no background event changes what the next key does: action keys are ignored
  for ``ARM_SECONDS`` after a refresh changed the selected card;
- the mouse follows the same rules: a click acts on the identity drawn under it
  at the last paint, re-checked against the model; a target that appeared or
  moved in a refresh ignores clicks for ``CLICK_ARM_SECONDS``; action clicks
  (go to pane, New agent, answer) are also subject to ``ARM_SECONDS``.
"""

from __future__ import annotations

import concurrent.futures
import dataclasses
import os
import threading
import time

from rich.segment import Segment
from rich.style import Style
import textual
from rich.cells import cell_len
from rich.control import Control
from textual import work
from textual.app import App, ComposeResult
from textual.containers import Vertical
from textual.geometry import Region
from textual.screen import ModalScreen
from textual.widgets import Input, Label, Static
from textual.binding import Binding
from textual.strip import Strip
from textual.widget import Widget

from . import (cockpit, config, core, herdr, home, inbox, inbox_link, machines, openkey, panels, protocol, render,
               theme, unseen)
from .collect import Collector
from .model import BLK, Swarm, live



TEXTUAL_CROP_VERIFIED = "8.2."  # the Textual versions _render_segments was checked against (tests/test_smooth.py)
try:  # a private Textual module: if it moves, panorama runs on Textual's own (slower) partial updates
    from textual import _compositor
except ImportError:
    _compositor = None
_textual_render_segments = getattr(getattr(_compositor, "ChopsUpdate", None), "render_segments", None)


def _render_segments(self, console) -> str:
    """Textual's ``ChopsUpdate.render_segments`` with each span cropped on both sides. Textual
    (8.2) crops a partly updated line only on the right and re-sends it from the widget's left
    edge, so a few pulsing cells in the middle of the one full-width Grid re-sent almost the
    whole screen at every colour tick. Same output otherwise (tests/test_smooth.py). A crop never
    starts inside a double-width character, and anything unexpected falls back to Textual's own."""
    try:
        move_to = Control.move_to
        out: list[str] = []
        last_y = self.spans[-1][0]
        for y, x1, x2 in self.spans:
            for end, (x, strip) in zip(self.chop_ends[y], self.chops[y].items(), strict=False):
                if strip is None or x > x2 or end <= x1:
                    continue
                if not (x2 > x >= x1 and end <= x2):
                    lo = x
                    if x1 > x:  # start at the character holding column x1, never in its middle
                        for seg in strip:
                            w = cell_len(seg.text)
                            if lo + w > x1:
                                for ch in seg.text:
                                    cw = cell_len(ch)
                                    if lo + cw > x1:
                                        break
                                    lo += cw
                                break
                            lo += w
                    strip = strip.crop(lo - x, min(end, x2) - x)
                    x = lo
                out.append(move_to(x, y).segment.text)
                out.append(strip.render(console))
            if y != last_y:
                out.append("\n")
        return "".join(out)
    except Exception:  # noqa: BLE001 - a changed Textual internal: its own (slower) output, never a dead view
        return _textual_render_segments(self, console)


if _textual_render_segments is not None and textual.__version__.startswith(TEXTUAL_CROP_VERIFIED):
    _compositor.ChopsUpdate.render_segments = _render_segments

REFRESH_SECONDS = 1.5
FRAME_SECONDS = 0.25  # a design frame: spinners turn and sweeps move one step
COLOUR_SECONDS = 0.05  # the colour tick (config animation_fps, default 20): pulses and blinks between frames
ARM_SECONDS = 0.4
FOLLOW_DEBOUNCE = 0.15  # per-tab panels: act on the last of a burst of focus events
ARROW_DOCK_SECONDS = 0.2  # docked panel: dock once the arrows rest this long (holding one docks once)
CLICK_ARM_SECONDS = 1.0  # a click target that appeared or moved in a refresh (herdr-inbox's row rule)
SCROLL_ARM_SECONDS = 0.3  # a click target the wheel moved under the pointer (trackpad momentum)
WHEEL_ROWS = 3

HELP = ("↑↓/jk move · →/l/enter open · ←/h back · g go to pane · t transcript path · "
        "a answer (inbox) · i inbox · f filter · q/esc close · "
        "mouse: click a card to select it, again to go to its pane; wheel scrolls a column, "
        "shift+wheel scrolls the columns sideways")


class Grid(Widget):
    """Paints the last rendered Screen."""

    DEFAULT_CSS = "Grid { width: 1fr; height: 1fr; }"

    def __init__(self) -> None:
        super().__init__()
        self.screen_data: render.Screen | None = None
        self._styles: dict[tuple, Style] = {}
        self._strips: dict[int, Strip] = {}  # render_line's rows, until show / restyle change them
        self._strips_of: render.Screen | None = None  # the screen and width they were made from
        self._strips_width = 0

    def style(self, fg: str, bg: str | None, bold: bool) -> Style:
        key = (fg, bg, bold)
        st = self._styles.get(key)
        if st is None:
            st = self._styles[key] = Style(color=fg, bgcolor=bg or render.BG, bold=bold)
        return st

    def on_click(self, event) -> None:
        if self.screen_data is not None:
            hit = self.screen_data.target_at(event.x, event.y)
            if hit:  # empty space: nothing
                self.app.click_target(*hit, cell=(event.x, event.y))

    def _wheel(self, event, rows: int) -> None:
        col = self.screen_data.column_at(event.x, event.y) if self.screen_data is not None else None
        if col is not None:
            event.stop()
            if getattr(event, "shift", False):
                self.app.hwheel(1 if rows > 0 else -1)  # shift+wheel: sideways
            else:
                self.app.wheel(col, rows)

    def _hwheel(self, event, cols: int) -> None:
        if self.screen_data is not None and self.screen_data.column_at(event.x, event.y) is not None:
            event.stop()
            self.app.hwheel(cols)

    def on_mouse_scroll_down(self, event) -> None:
        self._wheel(event, WHEEL_ROWS)

    def on_mouse_scroll_up(self, event) -> None:
        self._wheel(event, -WHEEL_ROWS)

    def on_mouse_scroll_right(self, event) -> None:
        self._hwheel(event, 1)

    def on_mouse_scroll_left(self, event) -> None:
        self._hwheel(event, -1)

    def on_mouse_move(self, event) -> None:
        self.app.hover(self.screen_data.card_at(event.x, event.y) if self.screen_data is not None else None)

    def on_leave(self, event) -> None:
        self.app.hover(None)

    def show(self, scr: render.Screen) -> None:
        """Paint ``scr``: only the spans that differ from the screen on display (all of it after
        a resize), so a frame where a spinner turned sends a few cells, not the whole view."""
        old, self.screen_data = self.screen_data, scr
        if old is None or (old.w, old.h) != (scr.w, scr.h):
            self.refresh()
            return
        if self._strips_of is old:
            self._strips_of = scr  # the rows that did not change keep their strips
        regions = []
        for y in range(scr.h):
            a, b = old.c[y], scr.c[y]
            if a == b:
                continue
            self._strips.pop(y, None)
            x0 = next(x for x in range(scr.w) if a[x] != b[x])
            x1 = next(x for x in range(scr.w - 1, -1, -1) if a[x] != b[x])
            regions.append(Region(x0, y, x1 - x0 + 1, 1))
        if regions:
            self.refresh(*regions)

    def restyle(self, frame: float) -> None:
        """The colour tick: recolour the animated cells in place and repaint only those."""
        scr = self.screen_data
        if scr is None:
            return
        spans = render.restyle(scr, frame)
        for y, _, _ in spans:
            self._strips.pop(y, None)
        if spans:
            self.refresh(*(Region(x0, y, x1 - x0, 1) for y, x0, x1 in spans))

    def render_line(self, y: int) -> Strip:
        scr = self.screen_data
        width = self.size.width
        if scr is None or y >= scr.h:
            return Strip([Segment(" " * width, self.style(render.FG, None, False))])
        if self._strips_of is not scr or self._strips_width != width:  # set directly, or resized
            self._strips.clear()
            self._strips_of, self._strips_width = scr, width
        strip = self._strips.get(y)
        if strip is None:
            segs = [Segment(text, self.style(fg, bg, bold)) for text, fg, bg, bold in scr.runs(y)]
            strip = self._strips[y] = Strip(segs).crop(0, width)
        return strip


class SwarmApp(App):
    CSS = "Screen { background: #101215; } NewAgentForm { background: rgba(16, 18, 21, 0.55); }"
    ENABLE_COMMAND_PALETTE = False
    BINDINGS = [
        Binding("up,k", "move(-1)", show=False), Binding("down,j", "move(1)", show=False),
        Binding("right,l", "right", show=False), Binding("left,h", "left", show=False),
        Binding("enter", "enter", show=False), Binding("g", "goto", show=False),
        Binding("t", "transcript", show=False), Binding("a", "answer", show=False),
        Binding("i", "inbox", show=False), Binding("f", "filter", show=False),
        Binding("question_mark", "help", show=False), Binding("n", "new_agent", show=False),
        Binding("q", "quit", show=False), Binding("escape", "escape", show=False),
        Binding("z", "zoom", show=False),
    ]

    def __init__(self, source=None, collector: Collector | None = None, focus_pane: str | None = None,
                 clock=time.time, live_refresh: bool = True, home_mode: bool | None = None,
                 cockpit_mode: bool | None = None):
        """``source()`` returns a Swarm (tests pass a fixed one); default: herdr + transcripts."""
        super().__init__()
        self.collector = collector or Collector()
        # saved herdr SSH machines (machines.py): their own threads, started by the real view's
        # first collect (a test's own source never starts them)
        self.remotes: machines.Remotes | None = None
        self.source = source or (lambda: self._merge_remotes(
            self.collector.collect(herdr.snapshot(), inbox.open_questions())))
        self.clock = clock
        self.live_refresh = live_refresh
        self.swarm = Swarm(herdr_ok=True)
        self.view = render.View()
        self.focus_pane = focus_pane or herdr.invocation_pane()
        self.armed_at = 0.0
        # the arming clock (keys, click cells, the wheel): monotonic time, held by tests so a
        # slow runner cannot let an arming window pass under them (#76)
        self.monotonic = time.monotonic
        self.after_exit = None  # callable run by the CLI once the popup has closed
        self.loaded = False
        self._collect_lock = threading.Lock()  # single flight: one collect at a time owns the tail state
        self.error: str | None = None
        self.arrived: dict[tuple, float] = {}  # cell (x, y) -> when a refresh changed what is drawn there
        self.scrolled: dict[tuple, float] = {}  # cell (x, y) -> until when: the wheel changed what is drawn there
        self.tracker = unseen.Tracker()  # what the operator has not seen yet (design turn 6)
        # In the home tab the view stays open: g/a/i/Enter act and keep it; in a popup they close it.
        self.home_mode = home.is_home() if home_mode is None else home_mode
        # Cockpit (experimental): selecting an orchestrator docks this pane left of its agent.
        # Read live on every action (None), so turning it off applies at the next Enter.
        self._cockpit_override = cockpit_mode
        self.my_pane = os.environ.get("HERDR_PANE_ID") if self.home_mode else None
        self.collector.self_pane = self.my_pane
        self.docking = False  # one dock at a time
        self.pane_focused = True  # panorama's pane has the keyboard (the terminal's focus in/out)
        self.herdr_focused_pane: str | None = None  # the pane herdr last reported focused
        self._unconfirmed: str | None = None  # a panel's announced focus, not yet confirmed by herdr
        self._follow_timer = None
        self._arrow_timer = None  # docked panel: the pending dock after the arrows settle
        self._arrow_again = False  # an arrow dock came due while a dock ran: look again after it
        self._parked_orch: str | None = None  # the orchestrator a parked cursor was last on
        self._stopping = False
        self.core: core.Core | None = None  # per-tab panels: this panorama publishes its state
        self.visible_tab: str | None = None  # the tab herdr last reported focused (per-tab panels)
        self.panels: panels.Manager | None = None  # per-tab panels: the panel manager (with the core)
        self.panel_panes: dict[str, str] = {}  # tab -> its panel, published to the viewers
        self.focus_log: cockpit.FocusLog | None = None  # set while the event stream is up
        # the animation's clock: the WALL clock, so the frame is a pure function of the time of
        # day and every panorama window (the home view, each tab's panel) shows an animation in
        # the same phase at the same moment, whenever it started and however long it was hidden
        self.anim_clock = time.time

    def compose(self) -> ComposeResult:
        self.grid = Grid()  # kept: query_one would search a modal screen on top (the form)
        yield self.grid

    def on_mount(self) -> None:
        if self.live_refresh and self.home_mode and cockpit.per_tab():
            self.core = core.Core.start(protocol.socket_path(), protocol.lock_path(),
                                        on_seen=lambda ids: self._to_ui(self._seen, ids),
                                        verify_pane=self._owned_pane, on_open=self._open_key,
                                        on_focusing=lambda pane: self._to_ui(self.focusing, pane),
                                        on_filter=lambda name: self._to_ui(self._filter_from_panel, name))
            if self.core is not None:
                self.panels = panels.Manager(
                    verified=self.core.verified_panes,
                    state_path=home.state_dir() / f"panels-{home.session_key()}.json")
        self.refresh_model()
        if self.live_refresh:
            self.set_interval(REFRESH_SECONDS, self.refresh_model)
            self.set_interval(self.colour_seconds(), self.animate)
            if self.home_mode:  # cockpit follow listens to herdr's focus events (read-only)
                threading.Thread(target=self._watch_focus, daemon=True).start()

    def on_unmount(self) -> None:
        self._stopping = True
        if self.core is not None:
            self.core.stop()
        if self.remotes is not None:
            self.remotes.stop()

    def _merge_remotes(self, local):
        if self.remotes is None:
            self.remotes = machines.Remotes.start()
        return self.remotes.merge(local)

    # -- model ---------------------------------------------------------------------

    @work(thread=True, group="refresh")
    def refresh_model(self) -> None:
        # Single flight: a tick that finds a collect still running is skipped. (A Textual
        # ``exclusive`` worker only marks the previous one cancelled; its thread keeps running.)
        if not self._collect_lock.acquire(blocking=False):
            return
        started = time.monotonic()
        try:
            swarm = self.source()
            marks = self.tracker.update(swarm, self.clock())  # file I/O and its lock: off the UI thread
        except Exception as exc:  # noqa: BLE001 - keep the last good model and say so
            self.call_from_thread(self.failed, f"{type(exc).__name__}: {exc}")
            return
        finally:
            self._collect_lock.release()
        self.call_from_thread(self.apply, swarm, started, marks)

    def failed(self, message: str) -> None:
        self.error = message[:200]
        self.log.error(f"refresh failed: {message}")
        self.repaint()

    def apply(self, swarm: Swarm, started: float | None = None, marks: dict | None = None) -> None:
        """``marks``: the unseen marks the collect worker computed for ``swarm`` (None: compute
        them here, as tests that apply a model directly do)."""
        self.error = None
        before = self.selected_card()
        old_cells = self._cells()
        self.swarm = swarm
        self.view.unseen = dict(self.tracker.update(swarm, self.clock()) if marks is None else marks)
        if not self.loaded:
            self.loaded = True
            self.initial_selection()
            self.armed_at = self.monotonic() + ARM_SECONDS
        else:
            self.resolve()
            self._update_dock()  # before the comparison: switching panels changes what keys do
            if self.selected_card() != before:
                self.armed_at = self.monotonic() + ARM_SECONDS
        self.repaint()
        self.publish()
        self.panels_pass()
        # Per-cell click arming: a cell whose target changed in this refresh (something appeared,
        # moved there, or another identity now sits where one was) ignores clicks for
        # CLICK_ARM_SECONDS: a click aimed before the refresh must not land on something else.
        # Only refreshes come here; frames and hover repaint without touching it, and the wheel
        # stamps its own shorter window (``_stamp_scrolled``).
        now = self.monotonic()
        self.arrived = {c: t for c, t in self.arrived.items() if now - t < CLICK_ARM_SECONDS}
        new_cells = self._cells()
        for cell, hit in new_cells.items():
            if old_cells.get(cell) != hit:
                self.arrived[cell] = now

    def publish(self) -> None:
        """Per-tab panels: the current model, unseen marks, focus and the visible tab to the
        panel viewers (a full snapshot each time, protocol v1)."""
        if self.core is None:
            return
        try:
            self.core.publish(protocol.state_message(
                self.swarm, self.view.unseen, self.herdr_focused_pane,
                [self.visible_tab] if self.visible_tab else None, self.panel_panes, self.view.filter))
        except protocol.TooLong as exc:
            self.log.warning(f"state not published: {exc}")

    # -- per-tab panels: the panel manager (panels.py) ------------------------------------

    def panels_pass(self) -> None:
        """After a refresh or a focus change: one manager pass in a worker (the manager skips it
        when one is running). The first pass is the start batch."""
        if self.panels is None or not self.loaded or self._stopping:
            return
        self.run_worker(self._panels_worker, thread=True, group="panels")

    def _panels_worker(self) -> None:
        try:
            m = self.panels
            if not cockpit.per_tab():  # switched off while running: the panels go, once
                if m.started:
                    if self.core is not None:
                        self.core.shutdown()  # the viewers exit; their panes close with them
                    m.remove_all()
                    m.started = False
                    self._to_ui(self._panels_now, {})
                return
            snap = herdr.snapshot()
            if snap is None:
                return
            sv = m.reconcile(snap, self.swarm, start=not m.started)
            if sv is not None:
                self._to_ui(self._panels_now, m.panels_map(sv))
        except Exception as exc:  # noqa: BLE001 - never kill the view; the next pass tries again
            msg = f"panels: {type(exc).__name__}: {exc}"
            self._to_ui(lambda: self.log.warning(msg))

    def _panels_now(self, mapping: dict) -> None:
        if mapping != self.panel_panes:
            self.panel_panes = mapping
            self.publish()

    def _open_key(self, req: dict) -> str:
        """The open key, from the core's socket (its reader thread): the manager decides, and
        "home" shows this home panorama."""
        if self.panels is None or not cockpit.per_tab():
            did = "home"
        else:
            snap = herdr.snapshot() or {}
            did = self.panels.open_key(snap, self.swarm)
        if did == "home" and self.my_pane:
            me = home.find(herdr.snapshot() or {}, prefer=self.my_pane)
            if me is not None:
                err = herdr.focus_tab(me.tab_id) or herdr.focus_pane(me.pane_id)
                return "failed" if err else "home"
            return "failed"
        return did

    def _tab_shown(self, tab_id: str) -> None:
        """herdr shows ``tab_id`` now: the per-tab panels animate only where they are seen."""
        if tab_id != self.visible_tab:
            self.visible_tab = tab_id
            self.publish()

    @staticmethod
    def _owned_pane(pane_id: str, pid: int) -> bool:
        """A viewer's hello names its pane: trusted only if herdr says that pane's foreground
        runs ``panorama panel`` as the connecting process itself (docs/DESIGN.md M2)."""
        return herdr.runs_panorama(pane_id, "panel", pid=pid) is True

    def _cells(self) -> dict[tuple, tuple]:
        scr = self.grid.screen_data if hasattr(self, "grid") else None
        return scr.cells() if scr else {}

    def selected_card(self):
        """What an action key would act on, as a comparable value."""
        o = render.find_orch(self.swarm, self.view.orch)
        t = render.find_thread(o, self.view.thread)
        a = render.find_agent(t, self.view.agent)
        return (self.view.dock, self.view.focus, o and o.pane_id, o and o.herdr_status, o and tuple(o.inbox), self.swarm.inbox_ok,
                t and t.id, a and a.id, a and a.status, a and tuple(a.question_ids), a and a.transcript)

    def initial_selection(self) -> None:
        cols = render.nav(self.swarm, self.view)
        if not cols[0]:
            return
        self.view.orch = self.focus_pane if self.focus_pane in cols[0] else cols[0][0]
        self.view.focus = 0
        self._default_children()

    def _default_children(self) -> None:
        """Select the remembered (else first) thread and agent below the current orchestrator."""
        cols = render.nav(self.swarm, self.view)
        remembered = self.view.remember_thread.get(self.view.orch or "")
        self.view.thread = remembered if remembered in cols[1] else (cols[1][0] if cols[1] else None)
        cols = render.nav(self.swarm, self.view)
        remembered = self.view.remember_agent.get(self.view.thread or "")
        self.view.agent = remembered if remembered in cols[2] else (cols[2][0] if cols[2] else None)

    def resolve(self) -> None:
        """After a refresh: keep every selection that still exists, park the ones that vanished."""
        cols = render.nav(self.swarm, self.view)
        if self.view.orch not in cols[0]:
            if self.view.orch is not None:
                self._parked_orch = self.view.orch  # where the arrows resume from (_resume)
            self.view.orch = None
        cols = render.nav(self.swarm, self.view)
        if self.view.thread not in cols[1]:
            self.view.thread = None
        cols = render.nav(self.swarm, self.view)
        if self.view.agent not in render.agent_ids(cols):
            self.view.agent = None

    def anim_frame(self) -> float:
        """The animation time now, in design frames of FRAME_SECONDS: ``anim_clock() /
        FRAME_SECONDS``, nothing else. Not a counter: two windows started at different times (or
        a panel that was hidden) read the same frame at the same instant, so switching tabs shows
        no phase jump. Each animation keeps its own period and per-element offset (render: a
        spinner's ``seq``, a sweep's ``seed``), so they are in sync between windows, not with
        each other. Precision: the time of day / 0.25 is about 7e9, where a float64 resolves
        about 1e-6 of a frame, so the colour glides (``frame % period`` into a sine) stay smooth;
        whole-frame animations take ``int(frame)``, an exact Python int."""
        return self.anim_clock() / FRAME_SECONDS

    def _step(self, frame: float) -> None:
        """Move the animation to ``frame`` from the cached screen: the cells that move (render.Anim:
        spinners, sweeps, pulses, blinks) are restyled in place and only they are repainted, no
        render and no layout. A frame where the docked panel's hold ran out repaints in full."""
        whole = int(frame) != int(self.view.frame)
        self.view.frame = frame
        if whole and hasattr(self, "grid"):
            dock = self.view.dock
            self._update_dock()
            if self.view.dock != dock:
                self.view.dock = dock
                self.repaint()
                return
        if hasattr(self, "grid"):
            self.grid.restyle(frame)

    @staticmethod
    def colour_seconds() -> float:
        """The colour tick's period: ``animation_fps`` from the config (2..30, default 20)."""
        try:
            return 1 / config.animation_fps()
        except (OSError, ValueError, TypeError):  # never let a broken config stop the view
            return COLOUR_SECONDS

    def animate(self) -> None:
        """The animation tick (every COLOUR_SECONDS): move to the frame the clock says now
        (``anim_frame``). At a new whole frame spinners and sweeps step; in between only the
        colours glide (pulses, blinks), and only if the screen has any. A low animation_fps, a
        late timer or a long stall needs no catching up: the next tick simply lands on the
        current frame. Nothing to do while nothing moves.

        It repaints from the cached model only, never a collect, and is not a background change:
        it touches neither the selection nor the arming. App focus is ignored, because herdr
        sends focus-out to a pane the operator merely typed past (the docked panel next to an
        agent is unfocused nearly all the time), not only a hidden one."""
        if not render.animated(self.swarm, self.view.unseen):
            return
        frame = self.anim_frame()
        if int(frame) == int(self.view.frame):
            scr = self.grid.screen_data if hasattr(self, "grid") else None
            if scr is None or not scr.smooth() or frame == self.view.frame:
                return
        self._step(frame)

    def repaint(self) -> None:
        grid = self.grid
        size = grid.size
        if size.width <= 0 or size.height <= 0:
            return
        self.view.now = self.clock()
        self.view.frame = self.anim_frame()  # a full paint draws the current phase, never a stale one
        self._update_dock()
        self.view.zoom_hint = self.home_mode and bool(self.swarm.docked) and self.can_zoom
        self.view.open_key = openkey.current()  # herdr's binding for the open key (#13)
        # design turn 10: the docked panel in herdr's theme, focused or not (purely visual)
        self.view.panel = theme.panel_look(theme.current(), self.pane_focused, render.FG, render.DIM,
                                           render.FAINT, (render.DOCK_HOV, render.BG)) if self.view.dock else None
        grid.show(self.decorate(render.render(self.swarm, self.view, size.width, size.height,
                                              stale=self.error or (self.swarm.errors[0] if self.swarm.errors else None))))

    can_zoom = True  # z zooms this pane to the full graph (not a per-tab panel)

    def decorate(self, scr):
        """A last touch on the rendered screen (a per-tab panel dims it without a core)."""
        return scr

    def herdr_focus(self, pane_id: str) -> None:
        """herdr says ``pane_id`` got the focus: panorama's panel is focused exactly when that is
        its own pane. The terminal's focus reports (``on_app_focus``) did not always arrive, e.g.
        for the open key's pane.focus (the operator's report), so herdr's own event counts too."""
        self.herdr_focused_pane = pane_id  # kept: panorama's own id can change after this (a dock)
        self._unconfirmed = None  # any focus herdr reports settles an announced one (focusing)
        if self.core is not None:
            self.publish()  # the panels' focus looks follow at once
        if not self.my_pane or not pane_id:
            return
        focused = pane_id == self.my_pane
        if focused != self.pane_focused:
            self.pane_focused = focused
            self.repaint()

    def focusing(self, pane_id: str) -> None:
        """A per-tab panel is about to switch tabs and put the keyboard on ``pane_id``: published
        now, so that panel paints its focused look while still hidden and is shown with it (the
        operator's report: the wrong focus colour for a frame). herdr's own event follows.
        Provisional: if no focus event from herdr comes within FOCUS_CONFIRM_SECONDS (the switch
        failed, the panel died), herdr's real focus is read and published (review of #44)."""
        self.herdr_focus(pane_id)
        self._unconfirmed = pane_id
        self.set_timer(FOCUS_CONFIRM_SECONDS, lambda: self._confirm_focus(pane_id))

    def _confirm_focus(self, pane_id: str) -> None:
        if self._unconfirmed != pane_id:
            return  # herdr reported a focus since: nothing to correct

        def read():
            real = (herdr.snapshot() or {}).get("focused_pane_id")
            self._to_ui(self._focus_read, pane_id, real)
        self.run_worker(read, thread=True, group="focus-confirm")

    def _focus_read(self, pane_id: str, real: str | None) -> None:
        if self._unconfirmed == pane_id and isinstance(real, str) and real:
            self.herdr_focus(real)

    def _refocus(self) -> None:
        """Panorama's pane id just changed (a dock or follow across workspaces gives it a new
        one): herdr's focus events for the new id may have arrived while the old id was still
        current, and none follows. Judge the last one again against the new id."""
        if self.herdr_focused_pane:
            self.herdr_focus(self.herdr_focused_pane)

    def on_app_focus(self, event) -> None:
        """The terminal reports focus in (herdr passes it to the focused pane): the panel's
        focused look. Visual only: nothing else depends on it."""
        self.pane_focused = True
        self.repaint()

    def on_app_blur(self, event) -> None:
        self.pane_focused = False
        self.repaint()

    def _update_dock(self) -> None:
        """Docked next to an agent (a per-tab panel) and narrower than the wide layout: the 3a
        panel, one column of orchestrator cards (focus on it). Decided in ``apply`` before the
        arming comparison, so a refresh that switches panels re-arms the keys. While a switch
        runs the panel is kept (herdr may resize the pane meanwhile)."""
        width = self.grid.size.width if hasattr(self, "grid") else 0
        self.view.dock = bool(self.swarm.docked) and width > 0 and (width < render.WIDE or self.docking)
        if self.view.dock and self.view.focus:
            self.view.focus = 0

    def cockpit_mode(self) -> bool:
        """Enter, a click and the arrows switch chats: a per-tab panel (PanelApp). The home
        panorama never docks: the moving cockpit is gone (Q187), per-tab panels do the docking."""
        return bool(self._cockpit_override)

    def on_resize(self, event) -> None:
        self.call_after_refresh(self.repaint)

    # -- navigation (operator-caused: never armed) -------------------------------------

    def check_action(self, action: str, parameters) -> bool | None:
        """Under the New agent form, the view's own keys are inert (arrows the form's inputs
        do not use must not move the cursor behind it)."""
        if isinstance(self.screen, NewAgentForm) and action in (
                "move", "right", "left", "enter", "goto", "transcript", "answer", "inbox", "filter", "new_agent",
                "zoom"):
            return False
        return True

    def _cur(self, col: int) -> str | None:
        """The selection in a column: the orchestrator, the thread, or (any depth) the agent."""
        v = self.view
        return v.orch if col == 0 else v.thread if col == 1 else v.agent

    def _thread(self):
        return render.find_thread(render.find_orch(self.swarm, self.view.orch), self.view.thread)

    def _focus(self, col: int) -> None:
        self.view.focus = col
        self.view.hcol = None  # a focus change scrolls sideways to the focus again

    def action_move(self, delta: int) -> None:
        cols = render.nav(self.swarm, self.view)
        col = self.view.focus
        items = cols[col] if col < len(cols) else []
        if not items:
            return
        cur = self._cur(col)
        if cur in items:
            idx = max(0, min(len(items) - 1, items.index(cur) + delta))
        else:
            idx = self._resume(col, items, delta)
        self._select(col, items[idx])
        if col == 0 and self._arrows_mode():
            self._arrow_dock_later()

    def _resume(self, col: int, items: list[str], delta: int) -> int:
        """A parked cursor: an orchestrator the filter hides (or the cursor parked off one)
        resumes from where it sits in the full list, to the next shown one in the arrow's
        direction; anything else resumes from the top."""
        anchor = self.view.orch if col == 0 and self.view.orch is not None else self._parked_orch
        order = [o.pane_id for o in self.swarm.orchestrators]
        if col != 0 or anchor not in order:
            return 0
        rank = {p: i for i, p in enumerate(order)}
        rank[render.NEW_ROW] = len(order)  # always last
        pos = rank[anchor]
        if delta > 0:
            return next((i for i, x in enumerate(items) if rank.get(x, -1) > pos), len(items) - 1)
        return next((i for i in range(len(items) - 1, -1, -1) if rank.get(items[i], len(order)) < pos), 0)

    def _arrows_mode(self) -> bool:
        """The docked panel with ``dock_on_arrows``: ↑/↓ switch the chats, Enter goes into one."""
        return self.view.dock and self.home_mode and self.cockpit_mode() and cockpit.arrows_setting()

    def _arrow_dock_later(self) -> None:
        if self._arrow_timer is not None:
            self._arrow_timer.stop()
        self._arrow_timer = self.set_timer(ARROW_DOCK_SECONDS, self._arrow_dock)

    def _arrow_dock(self) -> None:
        """The arrows rested on an orchestrator: dock next to it, the keyboard staying here. The
        same path as Enter and a click (identity re-checked, keep-focus's own guards); not for
        the docked one, + New agent, or one on another machine (its hint stays on Enter)."""
        self._arrow_timer = None
        if not self._arrows_mode() or self.view.focus != 0:
            return
        if self.docking:
            # first: during a dock (one across workspaces changes panorama's pane id, so the focus
            # checks below can be briefly wrong) the arrow is looked at again when it ends
            self._arrow_again = True
            return
        # the keyboard must still be on panorama: an arrow typed here, then the operator went into
        # a pane (the terminal's blur, or herdr reporting another pane focused): no switch
        if not self.pane_focused:
            return
        if self.herdr_focused_pane and self.my_pane and self.herdr_focused_pane != self.my_pane:
            return
        o = render.find_orch(self.swarm, self.view.orch) if self.view.orch != render.NEW_ROW else None
        if o is None or o.machine or o.pane_id == self.swarm.docked:
            return
        self._seen([a.id for a in o.agents()])  # docked next to it: all of it is in view
        self._dock(o.pane_id, True)

    def _select(self, col: int, ident: str) -> None:
        v = self.view
        v.wheel.clear()  # a selection change scrolls to the selected card again
        v.hcol = None
        if col == 0:
            if ident != v.orch:
                v.orch = ident
                self._default_children()
        elif col == 1:
            if ident != v.thread:
                v.thread = ident
                if v.orch:
                    v.remember_thread[v.orch] = ident
                cols = render.nav(self.swarm, v)
                remembered = v.remember_agent.get(ident)
                v.agent = remembered if remembered in cols[2] else (cols[2][0] if cols[2] else None)
        else:
            v.agent = ident
            t = self._thread()
            if t is not None:
                v.remember_agent[render.parent_key(t, ident)] = ident
        self.repaint()

    def action_right(self) -> None:
        if self.view.dock:
            return  # the docked panel has one column; Enter opens (docks next to) the agent
        v = self.view
        cols = render.nav(self.swarm, v)
        nxt = v.focus + 1
        if nxt >= len(cols) or not cols[nxt]:
            return
        if v.focus >= 2:
            # into the agents the selected one spawned (the one last selected there, else the first)
            t = self._thread()
            if t is None or not v.agent or v.agent.startswith(render.FIN_CARD):
                return
            kids = [i for i in cols[nxt] if render.parent_key(t, i) == v.agent]
            if not kids:
                return
            remembered = v.remember_agent.get(v.agent)
            self._select(nxt, remembered if remembered in kids else kids[0])
        elif nxt == 2 and v.agent and v.agent not in cols[2]:
            # a deeper agent stays selected: step into the depth-1 agent on its path
            path = render.path_ids(self._thread(), v.agent)
            self._select(2, path[0] if path and path[0] in cols[2] else cols[2][0])
        elif self._cur(nxt) not in cols[nxt]:
            self._select(nxt, cols[nxt][0])
        self._focus(nxt)
        self.repaint()

    def action_left(self) -> None:
        v = self.view
        if v.focus >= 3:
            # up to the agent that spawned the selected one
            t = self._thread()
            if t is not None and v.agent:
                key = render.parent_key(t, v.agent)
                if key != t.id:
                    self._select(v.focus - 1, key)
            self._focus(v.focus - 1)
            self.repaint()
        elif v.focus > 0:
            self._focus(v.focus - 1)
            self.repaint()

    def armed(self) -> bool:
        return self.monotonic() >= self.armed_at

    def action_enter(self) -> None:
        if not self.armed():
            return
        v = self.view
        if v.focus == 0 and v.orch == render.NEW_ROW:
            self.action_new_agent()
        elif v.focus == 0 and self.home_mode and v.orch:
            # home: Enter on an orchestrator goes to its pane; in the docked panel with arrows that
            # switch the chats, Enter is the way into the agent (what g does)
            self.action_goto(switch=not self._arrows_mode())
        elif v.focus == 1 and v.thread and v.thread.startswith(render.DONE_CARD):
            pane = v.thread[len(render.DONE_CARD):]
            v.done_open ^= {pane}
            self.repaint()
        elif v.focus >= 2 and v.agent and v.agent.startswith(render.FIN_CARD):
            key = v.agent[len(render.FIN_CARD):]
            self._seen(self._fin_agents(key))  # Enter on it: its folded agents are seen
            v.fin_open ^= {key}
            self.repaint()
        else:
            if v.focus >= 2 and v.agent:
                self._seen([v.agent])  # Enter on an unseen card: seen
            self.action_right()

    def _seen(self, ids) -> None:
        """The operator looked at these agents: their unseen marks clear at once; the state file
        is written by a worker thread (unseen.py)."""
        cleared = self.tracker.seen(ids)
        if cleared:
            for i in cleared:
                self.view.unseen.pop(i, None)
            self.run_worker(lambda: self.tracker.save_seen(cleared), thread=True, group="unseen")
            self.repaint()

    def _fin_agents(self, key: str) -> list[str]:
        """The agents folded into the "n finished" card of a group (thread id or agent id)."""
        t = self._thread()
        if t is None:
            return []
        e = next((e for e in render.tree_entries(t, self.view) if e.ident == render.FIN_CARD + key), None)
        return [a.id for a in (e.fin or [])] if e else []

    # -- actions ----------------------------------------------------------------------

    def _selection(self):
        """(orchestrator, thread, agent) the action keys act on; all None while the focused
        column's cursor is parked on nothing."""
        v = self.view
        if self._cur(v.focus) is None or v.orch == render.NEW_ROW:
            return None, None, None
        o = render.find_orch(self.swarm, v.orch)
        t = render.find_thread(o, v.thread)
        a = render.find_agent(t, v.agent) if v.focus >= 2 or t is not None else None
        return o, t, a

    def action_goto(self, switch: bool = False) -> None:
        """g: the selected orchestrator's pane (docked next to it with the cockpit on). Enter and
        the docked panel's one-click switch (``switch``) on another orchestrator than the docked
        one leave the keyboard on panorama (``dock_keep_focus``), so the next ↑/↓ and Enter browse
        on; on the docked one, and with g, focus goes into the agent."""
        if not self.armed():
            return
        if self._arrow_timer is not None:  # a key or click acts now: no settled-arrow dock after it
            self._arrow_timer.stop()
            self._arrow_timer = None
        o, _, _ = self._selection()
        if o is None:
            return
        self._seen([a.id for a in o.agents()])  # its pane gets the focus (or the dock): all of it is in view
        if o.machine:
            self._goto_remote(o)  # never a dock or a local focus: it lives on another server
            return
        if self.cockpit_mode():
            keep = (switch and self.view.dock and o.pane_id != self.swarm.docked
                    and cockpit.keep_focus_setting())
            self._dock(o.pane_id, keep)
            return
        err = herdr.focus_agent(o.pane_id)
        if err:
            self.notify(f"could not focus {o.pane_id}: {herdr.clean(err, 200)}", severity="warning")
        elif not self.home_mode:
            self.exit()

    def _goto_remote(self, o) -> None:
        """herdr's API cannot switch this herdr TUI to another machine. The operator's choice:
        focus the pane on that machine's own server (``herdr --machine <label> agent focus``,
        so it is in front once herdr shows that machine) and say where it is. Never a dock."""
        machine_id = o.machine_id or o.machine
        pane = machines.local_id(o.pane_id, machine_id)
        machine = o.machine
        msg = herdr.clean(f"{o.tab_label} is on {machine}: switch herdr to {machine} to see it", 300)

        def run():
            err = herdr.call("--machine", machine_id, "agent", "focus", pane)[1]
            text = herdr.clean(f"could not focus it on {machine}: {err}", 300) if err else msg
            self._to_ui(lambda: self.notify(text, title="another machine", timeout=8))
        self.run_worker(run, thread=True, group="remote-focus")

    # -- cockpit -------------------------------------------------------------------------

    # cockpit follow: the operator switched tabs some other way (tab bar, prefix+N, notifications)

    def _watch_focus(self) -> None:
        self.focus_log = cockpit.FocusLog()
        if self.panels is not None:
            self.panels.focus_log = self.focus_log  # the start batch's restore guard reads it
        for ev in herdr.events(["tab.focused", "pane.focused"], stop=lambda: self._stopping):
            if ev.get("type") in ("_connected", "_disconnected"):
                self.focus_log.live = ev["type"] == "_connected"
                if self.focus_log.live:
                    # where the keyboard is now: terminals report focus only on change, so a
                    # panorama started without it (the startup hook) would otherwise look focused
                    snap = herdr.snapshot() or {}
                    focused = snap.get("focused_pane_id")
                    if snap.get("focused_tab_id"):
                        try:
                            self.call_from_thread(self._tab_shown, snap["focused_tab_id"])
                        except Exception:  # noqa: BLE001 - the app is closing
                            return
                    if focused:
                        try:
                            self.call_from_thread(self.herdr_focus, focused)
                        except Exception:  # noqa: BLE001 - the app is closing
                            return
                continue
            if ev.get("type") in ("tab_focused", "pane_focused"):
                self.focus_log.record(ev)
                try:
                    if ev.get("type") == "tab_focused" and ev.get("tab_id"):
                        self.call_from_thread(self._tab_shown, ev["tab_id"])
                    if ev.get("type") == "pane_focused" and ev.get("pane_id"):
                        self.call_from_thread(self.herdr_focus, ev["pane_id"])
                    self.call_from_thread(self.focus_event)
                except Exception:  # noqa: BLE001 - the app is closing
                    return

    def focus_event(self) -> None:
        """A tab or pane got focus: wait FOLLOW_DEBOUNCE for the burst to settle (a tab switch
        sends tab_focused and pane_focused; prefix+1..9 can be pressed in a row), then act once
        on where the operator ended up."""
        if self._follow_timer is not None:
            self._follow_timer.stop()
        self._follow_timer = self.set_timer(FOLLOW_DEBOUNCE, self._follow_now)

    def _follow_now(self) -> None:
        self._follow_timer = None
        self.panels_pass()  # per-tab: the operator may have entered a tab that has no panel yet

    def action_zoom(self) -> None:
        """z: panorama full-screen in its tab (the 2a graph) and back to the docked panel."""
        if self.home_mode and self.my_pane:
            err = herdr.zoom_pane(self.my_pane)
            if err:
                self.notify(herdr.clean(err, 200), title="zoom", severity="warning")

    def _dock(self, pane_id: str, keep_focus: bool = False) -> None:
        """Switch to ``pane_id``'s chat: only a per-tab panel switches (PanelApp overrides
        this); the home panorama never docks (the moving cockpit is gone, Q187)."""

    def _to_ui(self, fn, *args) -> None:
        """A worker's result back to the UI thread; nothing if the view has closed meanwhile."""
        try:
            self.call_from_thread(fn, *args)
        except (RuntimeError, concurrent.futures.CancelledError):
            pass  # the app is shutting down (a panel's client thread can call in as it closes)

    def _docked(self, err: str | None) -> None:
        self.docking = False
        self._refocus()  # the dock may have given panorama a new pane id
        if self._arrow_again:  # the arrows settled elsewhere during this dock
            self._arrow_again = False
            self._arrow_dock_later()
        if err:
            self.notify(herdr.clean(err, 300), title="cockpit", severity="warning", timeout=8)

    def action_quit(self) -> None:
        """Quitting the home pane: a home tab the operator split loses the ◆ panorama label
        first, so nothing is left behind under it (issue #9; a crash is covered by the sweep
        in ``home.ensure``)."""
        if self.panels is not None:
            try:  # quitting panorama: its per-tab panels go with it (they come back with it)
                if self.core is not None:
                    self.core.shutdown()  # every viewer exits, layout.apply-built ones too
                self.panels.remove_all()
            except Exception as exc:  # noqa: BLE001 - quitting must always work
                self.log.warning(f"could not close the panels: {exc}")
        if self.home_mode:
            try:
                err = home.leave(self.my_pane)
                if err:
                    self.log.warning(f"could not release the home tab: {err}")
            except Exception as exc:  # noqa: BLE001 - quitting must always work
                self.log.warning(f"could not release the home tab: {exc}")
        self.exit()

    def action_transcript(self) -> None:
        if not self.armed():
            return
        o, _, a = self._selection()
        path = (a.transcript if a and a.transcript else None) or (o.transcript if o else None)
        self.notify(herdr.clean(path or "no transcript", 300), title="transcript", timeout=8)

    def action_answer(self) -> None:
        if not self.armed():
            return
        o, _, a = self._selection()
        if o is None:
            return
        if o.machine:
            self.notify(f"answering agents on {herdr.clean(o.machine, 60)} is not supported yet",
                        title="another machine", timeout=6)
            return
        if self.view.focus == 0:
            a = None  # on an orchestrator card: its own state and questions, not a remembered agent's
        if (a is not None and a.status == BLK) or (a is None and o.herdr_status == "blocked"):
            self.action_goto()  # blocking prompts are answered in the pane itself
            return
        if not self.swarm.inbox_ok:
            return
        if a is not None and a.question_ids:  # the agent's own question group
            self._open_inbox(question=min(a.question_ids))
        elif o.inbox:  # the orchestrator's oldest open group
            self._open_inbox(pane=o.pane_id)

    def action_inbox(self) -> None:
        if self.armed() and self.swarm.inbox_ok:
            self._open_inbox()

    def _open_inbox(self, question: int | None = None, pane: str | None = None) -> None:
        """herdr-inbox, deep-linked to ``question``'s group or ``pane``'s when it supports that
        (>= 0.6.0), else its plain popup."""
        def run():
            inbox_link.open_inbox(question=question, pane=pane)
        if self.home_mode:  # a tab is not modal: the inbox popup opens over it
            self.run_worker(run, thread=True, group="inbox")
            return
        # popups are modal: close this one first; the CLI opens the inbox afterwards
        self.after_exit = run
        self.exit()

    # -- mouse -------------------------------------------------------------------------

    def _shown(self, pane: str, tid: str | None = None, aid: str | None = None) -> bool:
        """Is this identity still selectable (exists, passes the filter, not folded away)?"""
        v = dataclasses.replace(self.view, orch=pane, thread=tid, agent=aid)
        cols = render.nav(self.swarm, v)
        return pane in cols[0] and (tid is None or tid in cols[1]) and (aid is None or aid in render.agent_ids(cols))

    def _point(self, pane: str, tid: str | None = None, aid: str | None = None) -> None:
        """Move the cursor to an identity (checked by ``_shown``), focusing its column."""
        if self.view.orch != pane:
            self._select(0, pane)
        if tid is not None:
            self._select(1, tid)
        col = 0 if tid is None else 1 if aid is None else 1 + render.agent_depth(self._thread(), aid)
        if aid is not None:
            self._select(col, aid)
        self._focus(col)

    def click_target(self, kind: str, ident: tuple, cell: tuple | None = None) -> None:
        """A click on something drawn at the last paint (``cell``: where). It acts on the
        identity drawn there, only if that identity is still shown, never when what is drawn
        at that cell changed in the last refresh (``CLICK_ARM_SECONDS``). Selection clicks only move the cursor; action
        clicks (go to pane, New agent, answer, transcript) are armed like their keys."""
        now = self.monotonic()
        if cell is not None and (now - self.arrived.get(cell, -1e9) < CLICK_ARM_SECONDS
                                 or now < self.scrolled.get(cell, 0.0)):
            return
        v = self.view
        if kind == "filter":
            self._set_filter(ident[0])
            return
        if kind == "new":
            if self.armed():
                self._select(0, render.NEW_ROW)
                v.focus = 0
                self.repaint()
                self.action_new_agent()
            return
        if kind in ("orch", "answer-orch"):
            if not self._shown(ident[0]):
                return
            again = v.orch == ident[0] and v.focus == 0
            if kind == "answer-orch":
                if self.armed():
                    self._point(ident[0])
                    self.repaint()
                    self.action_answer()
            elif self.view.dock:
                # the docked 3a panel: one click switches to it (what Enter then does, armed
                # and identity-checked like Enter; on the docked entry: focus its agent)
                self._point(ident[0])
                self.repaint()
                self.action_goto(switch=True)
            elif again:
                self.action_goto()  # the selected card, clicked again: its pane (g)
            else:
                self._point(ident[0])
                self.repaint()
            return
        if kind in ("done", "fin") and not self.armed():
            return  # folding is what Enter does: armed like Enter
        if kind == "done":
            card = render.DONE_CARD + ident[0]
            if not self._shown(ident[0], card):
                return
            self._point(ident[0], card)
            v.done_open ^= {ident[0]}  # what Enter does on it
            self.repaint()
            return
        if kind == "thread":
            if self._shown(ident[0], ident[1]):
                self._point(ident[0], ident[1])
                self.repaint()
            return
        if kind == "fin":
            key = ident[2] if len(ident) > 2 else ident[1]  # a nested group: the agent that spawned it
            fid = render.FIN_CARD + key
            if not self._shown(ident[0], ident[1], fid):
                return
            self._point(ident[0], ident[1], fid)
            self._seen(self._fin_agents(key))
            v.fin_open ^= {key}
            self.repaint()
            return
        if kind not in ("agent", "goto", "transcript", "answer") or len(ident) != 3:
            return
        if not self._shown(*ident):
            return
        self._seen([ident[2]])  # a click on an unseen card (or its actions): seen
        if kind == "agent":
            if (v.orch, v.thread, v.agent) == ident and v.focus == 1 + render.agent_depth(self._thread(), ident[2]):
                self.action_goto()  # the selected agent, clicked again: its pane (g)
            else:
                self._point(*ident)
                self.repaint()
            return
        if not self.armed():
            return
        self._point(*ident)
        self.repaint()
        {"goto": self.action_goto, "transcript": self.action_transcript, "answer": self.action_answer}[kind]()

    def hwheel(self, cols: int) -> None:
        """Scroll the columns sideways by ``cols`` (shift+wheel, a horizontal wheel). The focus
        stays; the next focus or selection change scrolls back to it."""
        v = self.view
        old = self._cells()
        v.hcol = max(0, (v.hcol if v.hcol is not None else v.hlo) + cols)
        self.repaint()
        self._stamp_scrolled(old)

    def _stamp_scrolled(self, old: dict) -> None:
        """A scroll slides other cards under a still pointer, and trackpad momentum keeps
        scrolling after the operator starts to click: every cell whose click target changed
        ignores clicks for SCROLL_ARM_SECONDS (shorter than a refresh's, so a deliberate
        scroll-then-click still works)."""
        now = self.monotonic()
        self.scrolled = {c: t for c, t in self.scrolled.items() if t > now}
        new = self._cells()
        for cell, hit in new.items():
            if old.get(cell) != hit:
                self.scrolled[cell] = now + SCROLL_ARM_SECONDS
        for cell in old.keys() - new.keys():
            self.scrolled[cell] = now + SCROLL_ARM_SECONDS

    def wheel(self, col: int, rows: int) -> None:
        """Scroll a column by the wheel. It stays where the wheel left it (no longer following
        the selection) until the selection changes; ``render.scroll`` clamps it."""
        v = self.view
        old = self._cells()
        v.offsets[col] = max(0, v.offsets.get(col, 0) + rows)
        v.wheel.add(col)
        self.repaint()
        self._stamp_scrolled(old)

    def hover(self, hit) -> None:
        """Highlight the card under the mouse; repaints only when that card changes."""
        if hit != self.view.hover:
            self.view.hover = hit
            self.repaint()

    def action_escape(self) -> None:
        if not self.home_mode:  # the home tab is not dismissed by a stray Esc; q still quits
            self.exit()

    def action_new_agent(self) -> None:
        """The + New agent form. Its default directory is the selected orchestrator's, read
        once when the form opens; nothing a refresh does changes the open form."""
        o = render.find_orch(self.swarm, self.view.orch)
        cwd = (o.cwd if o and o.cwd else None) or next((x.cwd for x in self.swarm.orchestrators if x.cwd), None)
        cwd = cwd or os.path.expanduser("~")
        width = self.grid.size.width if hasattr(self, "grid") else 0
        if self.home_mode and 0 < width < NEW_AGENT_MIN_WIDTH:
            # too narrow for the form (the docked panel's 46 columns): its own herdr popup,
            # over everything; the in-pane form where it fits
            t0 = time.time()
            err = herdr.open_new_agent(cwd)
            if not err:
                return
            if isinstance(err, herdr.NoReply):  # it may be open: a form here would be a second one
                self.log.warning(f"new agent popup: no reply from herdr ({err}); no form here")
                self.run_worker(lambda: self._popup_started(t0), thread=True, group="new-agent-popup")
                return
            self.log.warning(f"new agent popup: {err}")  # refused: the form in this pane
        self.push_screen(NewAgentForm(cwd), self._new_agent_done)

    def _popup_started(self, t0: float) -> None:
        """After a NoReply: wait for the popup's process to say it started; if it never does,
        tell the operator (nothing appeared, so n can be pressed again)."""
        end = time.monotonic() + POPUP_START_SECONDS
        while time.monotonic() < end:
            if NewAgentApp.started_since(t0):
                return
            time.sleep(0.05)
        if not NewAgentApp.started_since(t0):
            self._to_ui(lambda: self.notify(POPUP_DID_NOT_OPEN, title="new agent", severity="warning", timeout=6))

    def _new_agent_done(self, name: str | None) -> None:
        if not name:
            return
        self.notify(f"started {name}", title="new agent", timeout=6)
        if not self.home_mode:
            self.exit()  # the new tab has focus; a popup over it would hide it

    def action_filter(self) -> None:
        f = render.FILTERS
        self._set_filter(f[(f.index(self.view.filter) + 1) % len(f)])

    def _set_filter(self, name: str) -> None:
        """The operator chose a filter here. Per-tab panels: every view shows it (the core
        publishes it; a panel sends it to the core, panel.py)."""
        self._apply_filter(name)
        self.publish()

    def _filter_from_panel(self, name: str) -> None:
        """A per-tab panel chose a filter (the core's reader): this view takes it, and the
        state published with it tells every other panel."""
        if name not in render.FILTERS:
            return
        if name != self.view.filter:
            self._apply_filter(name)
        self.publish()

    def _filter_to(self, name: str) -> None:
        """The filter changes: a selection it hides is parked (resolve)."""
        self.view.filter = name
        self.view.wheel.clear()
        self.resolve()

    def _apply_filter(self, name: str) -> None:
        self._filter_to(name)
        cols = render.nav(self.swarm, self.view)
        if self.view.orch is None and cols[0]:
            self.view.orch = cols[0][0]
            self._default_children()
        self.repaint()

    def action_help(self) -> None:
        self.notify(HELP, title="keys", timeout=10)


def live_count(swarm: Swarm) -> int:
    return sum(1 for o in swarm.orchestrators for a in o.agents() if live(a.status))


FOCUS_CONFIRM_SECONDS = 0.75  # an announced focus herdr has not confirmed by then is re-read
NEW_AGENT_MIN_WIDTH = 80  # the form is 76 columns wide (plus its margin): narrower panes use a popup
POPUP_START_SECONDS = 3.0  # after an unanswered popup request: how long its process may take to start
POPUP_DID_NOT_OPEN = "herdr did not open the New agent popup; press n again"  # the operator's wording (Q154)


class NewAgentApp(App):
    """``panorama new-agent``: the + New agent form alone, in its own herdr popup (opened from
    a pane too narrow for it). The same form, arming, validation and agent creation; the popup
    closes when the form is done or cancelled."""

    CSS = "Screen { background: #101215; }"

    def __init__(self, cwd: str | None = None):
        super().__init__()
        self.cwd = cwd or os.environ.get("PANORAMA_NEW_AGENT_CWD") or os.path.expanduser("~")

    def on_mount(self) -> None:
        self.mark_started()
        self.push_screen(NewAgentForm(self.cwd), lambda name: self.exit())

    @staticmethod
    def _marker():
        return home.state_dir() / f"new-agent-{home.session_key()}.started"

    @classmethod
    def mark_started(cls) -> None:
        """The popup's process is up: the opener (after an unanswered request) looks for this."""
        try:
            cls._marker().parent.mkdir(parents=True, exist_ok=True)
            cls._marker().write_text(str(time.time()))
        except OSError:
            pass

    @classmethod
    def started_since(cls, t0: float) -> bool:
        try:
            return float(cls._marker().read_text() or 0) >= t0
        except (OSError, ValueError):
            return False


class NewAgentForm(ModalScreen):
    """Name (required; slugified to herdr's agent-name rule, also the tab label), working
    directory, optional first prompt. Enter on the last field or ctrl+s submits; Esc
    cancels. Submit is armed like every view (ARM_SECONDS after it appears) and runs the
    herdr calls in a worker; the form stays until they finish."""

    DEFAULT_CSS = """
    NewAgentForm { align: center middle; background: rgba(16, 18, 21, 0.55); }
    #form { width: 76; height: auto; padding: 1 2; background: #171a1f; border: round #80868f; }
    #form Label { color: #80868f; margin-top: 1; }
    #title { color: #eceef1; text-style: bold; }
    #slug { color: #e2cf9c; height: 1; }
    #status { color: #ffb547; height: auto; margin-top: 1; }
    #hint { color: #4d535c; margin-top: 1; }
    """
    BINDINGS = [Binding("escape", "cancel", show=False), Binding("ctrl+s", "submit", show=False)]

    def __init__(self, cwd: str):
        super().__init__()
        self.default_cwd = cwd
        self.armed_at = time.monotonic() + ARM_SECONDS
        self.busy = False

    def compose(self) -> ComposeResult:
        with Vertical(id="form"):
            yield Static("+ New agent", id="title")
            yield Label("Name (also the tab label)")
            yield Input(placeholder="billing-api", id="name")
            yield Static("", id="slug")
            yield Label("Working directory")
            yield Input(value=self.default_cwd, id="cwd")
            yield Label("First prompt (optional)")
            yield Input(placeholder="what should it start with?", id="prompt")
            yield Static("", id="status")
            yield Static("enter on the last field or ctrl+s: start · tab: next field · esc: cancel", id="hint")

    def on_mount(self) -> None:
        self.query_one("#name", Input).focus()

    def on_input_changed(self, event: Input.Changed) -> None:
        if event.input.id == "name":
            slug = home.slugify(event.value)
            self.query_one("#slug", Static).update(f"agent name: {slug}" if slug else "")

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "prompt":
            self.action_submit()
        else:
            self.focus_next()

    def action_cancel(self) -> None:
        if not self.busy:
            self.dismiss(None)

    def action_submit(self) -> None:
        if self.busy or time.monotonic() < self.armed_at:
            return
        name = self.query_one("#name", Input).value
        if not home.NAME_RULE.match(home.slugify(name)):
            self.query_one("#status", Static).update("The name needs at least one letter.")
            return
        cwd = self.query_one("#cwd", Input).value.strip()  # read here, on the UI thread:
        prompt = self.query_one("#prompt", Input).value  # the values are frozen at submit
        if cwd and not os.path.isdir(cwd):
            self.query_one("#status", Static).update(f"No such directory: {herdr.clean(cwd, 200)}")
            return
        self.busy = True
        for field in self.query(Input):
            field.disabled = True
        self.query_one("#status", Static).update("starting Claude Code…")
        self.run_worker(lambda: self._start(name, cwd, prompt), thread=True)

    def _start(self, name: str, cwd: str, prompt: str) -> None:
        slug, err = home.new_agent(name, cwd, prompt)
        self.app.call_from_thread(self._finished, slug, err)

    def _finished(self, slug: str | None, err: str | None) -> None:
        self.busy = False
        for field in self.query(Input):
            field.disabled = False
        if err and not slug:
            self.query_one("#status", Static).update(herdr.clean(err, 300))
            return
        if err:
            self.app.notify(herdr.clean(err, 300), severity="warning")
        self.dismiss(slug)
