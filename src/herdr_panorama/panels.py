"""Per-tab panels, part 2: the panel manager, inside the core (the home panorama). See
docs/DESIGN.md "Per-tab panels", "The panel manager" and the operator's answers.

Which tabs get a panel: tabs holding a local Claude orchestrator in panorama's workspace (not
the home tab; with ``cockpit_follow_tabs = "single"`` only tabs where it is the only pane).

Placement is ``place`` and nothing else: a split right of the agent without focus, then a
swap with the agent as the swap's source (herdr focuses a swap's source and shows its tab),
then the width. When it runs:

- **At core start, one batch** for every tab without a panel. The swaps pull the operator
  through those tabs (accepted, Q142), so the batch remembers where they were and puts them
  back, unless focus events that are not the batch's own show they went somewhere themselves.
  The batch runs only here: while it runs, keys go to whichever agent a swap focused, and
  herdr 0.9.3 has no way to hold or route input (``pane.input.set`` only sets right-click).
- **Later, only in the tab the operator is in**: a new orchestrator tab gets its panel on
  their first visit, and the swap keeps them on the agent they are in. If they leave before
  the swap, the panel stays on the right until their next visit.

What is ours (closed by the manager): a pane whose foreground runs ``panorama panel`` (herdr's
``pane process-info``), and that the core opened itself or whose viewer said a verified hello.
A pane that only looks like a panel (argv, label) is never closed; it does count as the tab's
panel, so no second one is made.

A panel the operator closes stays closed for its tab (kept in the state dir) until the open key
asks for it there again. A tab whose orchestrator went away loses its panel after
``REMOVE_AFTER`` (Claude exiting and starting again keeps it).
"""

from __future__ import annotations

import contextlib
import json
import logging
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

from . import cockpit, herdr, home

log = logging.getLogger("herdr_panorama.panels")

REMOVE_AFTER = 30.0  # a tab without an orchestrator this long loses its panel
CREATING_SECONDS = 15.0  # a create in flight: herdr's snapshot may not show the pane yet
NOT_PANEL_SECONDS = 30.0  # how long a "not a panel" answer for a pane is believed
PANEL_SECONDS = 30.0  # how long a "runs our viewer" answer is believed (a pane never becomes another)
CRASH_LIMIT = 3  # panels of one tab that died on their own before the manager stops making them
LOCK_SECONDS = 2.0


@dataclass
class Survey:
    """One look at herdr: the tabs that should have a panel and the panels there are."""
    targets: dict[str, dict] = field(default_factory=dict)  # tab -> the orchestrator's pane
    panels: dict[str, list[str]] = field(default_factory=dict)  # tab -> panel panes (argv), herdr's order
    unknown: set[str] = field(default_factory=set)  # tabs with a pane herdr could not check
    tabs: set[str] = field(default_factory=set)  # every tab in the snapshot
    focused_tab: str | None = None
    focused_pane: str | None = None
    home_tab: str | None = None


def _operator_moved(events: list[dict], expected: list[tuple[str, str]]) -> str | None:
    """The first focus event that is not the batch's own: the batch's swaps cause ``expected``
    in order (herdr may leave one out, e.g. no tab event when that tab is already shown); an
    event that is not the next of them, or a later one, is the operator's."""
    i = 0
    for ev in events:
        key = (ev.get("type"), ev.get("pane_id") if ev.get("type") == "pane_focused" else ev.get("tab_id"))
        if key[0] not in ("tab_focused", "pane_focused") or not key[1]:
            continue
        j = next((k for k in range(i, len(expected)) if expected[k] == key), None)
        if j is None:
            return key[1]
        i = j + 1
    return None


def _crashed(pane_id: str) -> bool:
    """The viewer left a crash marker (panel.mark_crash); it is used up here."""
    from .panel import crash_marker
    path = crash_marker(pane_id)
    try:
        path.unlink()
        return True
    except FileNotFoundError:
        return False
    except OSError:
        return True


class Manager:
    """The panel manager. ``verified()`` returns the pane ids whose viewers said a verified
    hello (the core's connected viewers). ``focus_log`` is the home panorama's focus event log
    (cockpit.FocusLog) or None."""

    def __init__(self, verified=lambda: set(), focus_log=None, state_path: Path | None = None,
                 clock=time.monotonic):
        self.verified = verified
        self.focus_log = focus_log
        self.clock = clock
        self.state_path = state_path
        self.created: dict[str, str] = {}  # panel pane -> tab, opened by this core
        self.creating: dict[str, float] = {}  # tab -> until when a create counts as in flight
        self.right: set[str] = set()  # tabs whose new panel could not be swapped left yet
        self.orphaned: dict[str, float] = {}  # tab -> since when it has no orchestrator
        self.closed: set[str] = self._load_closed()
        self._not_panel: dict[str, float] = {}  # pane -> until when "not a panel" is believed
        self._panel: dict[str, float] = {}  # pane -> until when "a panel" is believed
        self.connected_once: set[str] = set()  # our panels whose viewer said a verified hello
        self.crashes: dict[str, int] = {}  # tab -> panels that died on their own
        self.broken: set[str] = set()  # tabs given up on (CRASH_LIMIT), for this core's life
        self._mu = threading.Lock()
        self.started = False  # the start batch ran
        # around the start batch: a no-op now; the slot for something that holds the keyboard
        # while the swaps pull the operator through the tabs (Q153)
        self.guard = contextlib.nullcontext

    # -- state -----------------------------------------------------------------------------

    def _load_closed(self) -> set[str]:
        try:
            data = json.loads(self.state_path.read_text()) if self.state_path else {}
        except (OSError, ValueError):
            data = {}
        return {t for t in data.get("closed") or [] if isinstance(t, str)} if isinstance(data, dict) else set()

    def _save_closed(self) -> None:
        if not self.state_path:
            return
        try:
            self.state_path.parent.mkdir(parents=True, exist_ok=True)
            tmp = self.state_path.with_suffix(".tmp")
            tmp.write_text(json.dumps({"closed": sorted(self.closed)}))
            tmp.replace(self.state_path)
        except OSError as exc:
            log.warning("could not save the closed panels: %s", exc)

    # -- looking ---------------------------------------------------------------------------

    def is_panel(self, pane_id: str) -> bool | None:
        """Whether the pane's foreground runs ``panorama panel`` (None: herdr could not say)."""
        if pane_id in self.created:
            return True
        if self._not_panel.get(pane_id, 0.0) > self.clock():
            return False
        if self._panel.get(pane_id, 0.0) > self.clock():
            return True
        runs = herdr.runs_panorama(pane_id, "panel")
        if runs is False:
            self._not_panel[pane_id] = self.clock() + NOT_PANEL_SECONDS
        elif runs:
            self._panel[pane_id] = self.clock() + PANEL_SECONDS
        return runs

    def survey(self, snap: dict, swarm) -> Survey:
        """Which tabs should have a panel (from the model's local orchestrators, checked against
        the snapshot) and which panels exist."""
        sv = Survey(focused_tab=snap.get("focused_tab_id"), focused_pane=snap.get("focused_pane_id"))
        panes = [p for p in snap.get("panes") or [] if isinstance(p, dict) and p.get("pane_id")]
        by_id = {p["pane_id"]: p for p in panes}
        sv.tabs = {p.get("tab_id") for p in panes if p.get("tab_id")}
        me = home.find(snap)
        sv.home_tab = me.tab_id if me else None
        workspace = (by_id.get(me.pane_id) or {}).get("workspace_id") if me else None
        for o in getattr(swarm, "orchestrators", None) or []:
            p = by_id.get(o.pane_id) if not o.machine else None
            if p is None or p.get("agent") != "claude" or not p.get("tab_id"):
                continue
            if p["tab_id"] == sv.home_tab or (workspace and p.get("workspace_id") != workspace):
                continue
            sv.targets.setdefault(p["tab_id"], p)
        for tab in set(sv.targets) | set(self.created.values()):
            for p in panes:
                if p.get("tab_id") != tab or p.get("agent") or p["pane_id"] == (me.pane_id if me else None):
                    continue
                runs = self.is_panel(p["pane_id"])
                if runs:
                    sv.panels.setdefault(tab, []).append(p["pane_id"])
                elif runs is None:
                    sv.unknown.add(tab)
        if cockpit.follow_tabs() == "single":
            for tab, agent in list(sv.targets.items()):
                others = [p for p in panes if p.get("tab_id") == tab and p["pane_id"] != agent["pane_id"]
                          and p["pane_id"] not in sv.panels.get(tab, [])]
                if others:
                    del sv.targets[tab]
        return sv

    def ours(self, pane_id: str) -> bool:
        """Provably ours, so the manager may close it: it runs our viewer (checked by the
        survey) and this core opened it or its viewer said a verified hello."""
        return pane_id in self.created or pane_id in self.verified()

    def panels_map(self, sv: Survey) -> dict[str, str]:
        """tab -> its panel, for the viewers' navigation (a connected one first)."""
        live = self.verified()
        return {tab: next((p for p in ps if p in live), ps[0]) for tab, ps in sv.panels.items() if ps}

    # -- placing ---------------------------------------------------------------------------

    def place(self, tab: str, agent: dict, lazy: bool) -> str | None:
        """The one placement: split right of the agent without focus, swap it left with the agent
        as the swap's source, set its width. ``lazy`` (the operator is in this tab): look again
        right before the swap, and if they have left, leave the panel on the right for now; if
        they were on another pane of the tab, give it the focus back. The panel's pane id."""
        self.creating[tab] = self.clock() + CREATING_SECONDS
        pane, err = herdr.open_panel(agent["pane_id"])
        if pane is None:
            log.warning("no panel for %s: %s", tab, err)
            return None
        panel = pane["pane_id"]
        self.created[panel] = tab
        was = None
        if lazy:
            now = herdr.snapshot() or {}
            if now.get("focused_tab_id") != tab:
                self.right.add(tab)
                cockpit.fit(panel, tab)
                return panel
            was = now.get("focused_pane_id")
        self._swap_left(tab, agent["pane_id"], panel)
        if lazy and was and was not in (agent["pane_id"], panel):
            herdr.focus_pane(was)
        return panel

    def _swap_left(self, tab: str, agent: str, panel: str) -> None:
        err = herdr.swap_panes(agent, panel)  # the agent is the source: it keeps the focus
        if err:
            log.warning("the panel in %s stays right of its agent: %s", tab, err)
            self.right.add(tab)
        else:
            self.right.discard(tab)
        cockpit.fit(panel, tab)

    def batch(self, missing: list[tuple[str, dict]], sv: Survey) -> list[str]:
        """The start batch: every missing panel, then the operator back where they were, unless
        a focus event that is not one of the batch's own shows they went elsewhere."""
        t0 = time.monotonic()
        where = (sv.focused_tab, sv.focused_pane)
        expected: list[tuple[str, str]] = []  # the focus events the batch's own swaps cause, in order
        made = []
        with self.guard():
            for tab, agent in missing:
                panel = self.place(tab, agent, lazy=False)
                if panel:
                    made.append(panel)
                    expected += [("tab_focused", tab), ("pane_focused", agent["pane_id"])]
        if not made or not where[0]:
            return made
        if self.focus_log is not None and self.focus_log.live:
            last = expected[-1]
            self.focus_log.wait_for(t0, lambda d: (d.get("type"), d.get("pane_id")) == last, cockpit.SYNC_SECONDS)
            moved = _operator_moved(self.focus_log.since(t0), expected)
            if moved:
                log.info("panel batch: the operator went to %s; left there", moved)
                return made
        err = herdr.focus_tab(where[0])
        if not err and where[1]:
            err = herdr.focus_pane(where[1])
        if err:
            log.warning("panel batch: could not put the focus back: %s", err)
        return made

    # -- the loop --------------------------------------------------------------------------

    def reconcile(self, snap: dict, swarm, start: bool = False) -> Survey | None:
        """One pass: forget what is gone, notice panels the operator closed, close duplicates and
        orphans, and place what is missing (the start batch, else only the operator's tab).
        Never two at once; returns the survey, or None when another pass is running."""
        if not self._mu.acquire(blocking=False):
            return None
        try:
            with home._lock(LOCK_SECONDS):
                return self._reconcile(snap, swarm, start)
        except home.LockBusy:
            return None
        finally:
            self._mu.release()

    def _reconcile(self, snap: dict, swarm, start: bool) -> Survey:
        now = self.clock()
        sv = self.survey(snap, swarm)
        present = {p.get("pane_id") for p in snap.get("panes") or [] if isinstance(p, dict)}
        live = self.verified()
        for tab, ps in sv.panels.items():  # adopted: a verified viewer's panel (after a core restart)
            for p in ps:
                if p in live:
                    self.created.setdefault(p, tab)
        self.connected_once |= live & set(self.created)
        for panel, tab in list(self.created.items()):
            if panel in present:
                continue
            del self.created[panel]
            self.creating.pop(tab, None)  # that create is over
            if tab not in sv.targets:
                continue
            # gone while its orchestrator is still there: the operator closed it, unless its viewer
            # died on its own (never connected, e.g. a broken environment, or left a crash marker)
            if panel not in self.connected_once or _crashed(panel):
                n = self.crashes[tab] = self.crashes.get(tab, 0) + 1
                log.warning("the panel in %s died on its own (%d of %d)%s", tab, n, CRASH_LIMIT,
                            "; no more panels there" if n >= CRASH_LIMIT else "")
                if n >= CRASH_LIMIT:
                    self.broken.add(tab)
                continue
            self.closed.add(tab)
            self._save_closed()
        if self.closed - sv.tabs:  # tabs that no longer exist
            self.closed &= sv.tabs
            self._save_closed()
        self.creating = {t: until for t, until in self.creating.items() if until > now and t not in sv.panels}
        self.right &= set(sv.panels)
        # duplicates: keep one (a connected one first), close the provable rest
        for tab, ps in sv.panels.items():
            if len(ps) > 1:
                keep = next((p for p in ps if p in live), ps[0])
                for p in ps:
                    if p != keep and self.ours(p) and not herdr.close_plugin_pane(p):
                        self.created.pop(p, None)
                        log.info("closed a second panel in %s", tab)
                sv.panels[tab] = [keep]
        # orphans: a tab whose orchestrator went away keeps its panel for REMOVE_AFTER
        for tab in list(sv.panels):
            if tab in sv.targets:
                self.orphaned.pop(tab, None)
                continue
            since = self.orphaned.setdefault(tab, now)
            if now - since >= REMOVE_AFTER:
                for p in sv.panels[tab]:
                    if self.ours(p) and not herdr.close_plugin_pane(p):
                        self.created.pop(p, None)
                self.orphaned.pop(tab, None)
                del sv.panels[tab]
        for tab in list(self.orphaned):
            if tab not in sv.panels:
                del self.orphaned[tab]
        # placing
        missing = [(tab, agent) for tab, agent in sv.targets.items()
                   if tab not in sv.panels and tab not in sv.unknown and tab not in self.closed
                   and tab not in self.creating and tab not in self.broken]
        if start:
            self.started = True
            for panel in self.batch(missing, sv):
                sv.panels.setdefault(self.created[panel], []).append(panel)
        elif sv.focused_tab:
            tab = sv.focused_tab
            target = dict(missing).get(tab)
            if target is not None:
                panel = self.place(tab, target, lazy=True)
                if panel:
                    sv.panels.setdefault(tab, []).append(panel)
            elif tab in self.right and tab in sv.targets and sv.panels.get(tab):
                self._swap_left(tab, sv.targets[tab]["pane_id"], sv.panels[tab][0])
        return sv

    # -- the operator ----------------------------------------------------------------------

    def open_key(self, snap: dict, swarm) -> str:
        """The open key in per-tab mode: in an orchestrator tab with a panel, focus the panel;
        in one without (closed by hand, or not made yet), clear its closed mark, make it and
        focus it; anywhere else: "home" (the caller shows the home tab)."""
        with self._mu:
            try:
                with home._lock(LOCK_SECONDS):
                    sv = self.survey(snap, swarm)
                    tab = sv.focused_tab
                    if tab not in sv.targets:
                        return "home"
                    if tab in self.closed:
                        self.closed.discard(tab)
                        self._save_closed()
                    panel = self.panels_map(sv).get(tab)
                    if panel is None:
                        panel = self.place(tab, sv.targets[tab], lazy=True)
                    if panel is None:
                        return "failed"
            except home.LockBusy:
                return "busy"
        err = herdr.focus_pane(panel)
        return "failed" if err else "panel"

    def remove_all(self, snap: dict | None = None) -> int:
        """Per-tab off (the cockpit off or ``"moving"``), or the home panorama quits: close every
        panel provably ours. How many were closed."""
        snap = snap if snap is not None else herdr.snapshot() or {}
        n = 0
        with self._mu:
            for p in snap.get("panes") or []:
                pid = p.get("pane_id") if isinstance(p, dict) else None
                if pid and not p.get("agent") and self.ours(pid) and self.is_panel(pid) \
                        and not herdr.close_plugin_pane(pid):
                    n += 1
            self.created.clear()
            self.right.clear()
        return n
