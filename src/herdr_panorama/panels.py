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

After herdr itself stops unexpectedly, its session restore brings every pane back with its pane
id and label but as a plain login shell: it does not start a pane's command again (only a live
handoff keeps the process). herdr 0.9.3-0.9.5 has no way to run a command in an existing pane
short of typing into its shell, which panorama never does, so such a pane is closed and the
usual rules make a new panel. The manager records each panel of ours (pane id and herdr's
terminal id) in the state dir; quitting and switching off clear it, so only an unclean end
leaves a record. A pane is closed as a restored panel only when all of these hold:

- its pane id is recorded (a recorded pane missing from a pass is forgotten at once, so a
  fresh server that numbers panes anew never matches an old record);
- herdr gave it a different terminal id. A crash restore does that, but so does a live
  handoff, where the panel keeps running: that case is told apart only by the next check;
- it still has the panel's label, no agent, and is not focused: not the focused pane, not the
  pane its tab remembers as focused (``layouts[].focused_pane_id``: the operator was last in it,
  before the crash or since), and no focus event for it in this core's whole focus log (checked
  again right before the close). Without a live focus stream nothing is decided;
- ``pane process-info``: one foreground process, the pane's own ``shell_pid``, whose argv is
  a plain shell (an ``exec``-ed program keeps the pid but not the argv);
- ``pgrep -P``: the shell has no child (no background job), and ``ps -o etime``: it is younger
  than ``RESTORED_SECONDS``;
- a second ``pane process-info`` right before the close still says the same.

Any other answer (only "herdr cannot say yet" excepted) gives the pane up for good, and so does
not deciding within ``RESTORE_PASSES`` passes of the first pass that saw the restore: leaving a
shell is safe, closing a pane the operator used is not. ``RESTORED_SECONDS`` is short (15 s)
so that the operator has had no time to use the shell unseen before the core's first look; a
core that starts later than that after herdr leaves the restored shells in place. Residual: herdr has no
conditional close, so a command typed in the milliseconds between the last look and the
close goes with the pane. The age and child checks assume herdr runs in this pid namespace;
macOS computes ``etime`` from the wall clock.
"""

from __future__ import annotations

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
RESTORED_SECONDS = 15.0  # a restored panel's shell is at most this old when it is closed
RESTORE_PASSES = 3  # passes, from the first that sees a restore, on which restored panels are judged


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


def _tab_focus(snap: dict, pane_id: str) -> bool:
    """The pane is the one its tab remembers as focused (``layouts[].focused_pane_id``): the
    operator was last in it, before the crash or since the restore; either way, not closed."""
    return any(isinstance(lay, dict) and lay.get("focused_pane_id") == pane_id for lay in snap.get("layouts") or [])


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
        saved = self._load()
        self.closed: set[str] = {t for t in saved.get("closed") or [] if isinstance(t, str)}
        # our panels as last seen running: pane -> {"tab", "terminal"} (herdr's terminal id)
        self.record: dict[str, dict] = {p: r for p, r in (saved.get("panes") or {}).items()
                                        if isinstance(p, str) and isinstance(r, dict)} \
            if isinstance(saved.get("panes"), dict) else {}
        self._not_panel: dict[str, float] = {}  # pane -> until when "not a panel" is believed
        self._panel: dict[str, float] = {}  # pane -> until when "a panel" is believed
        self.connected_once: set[str] = set()  # our panels whose viewer said a verified hello
        self.crashes: dict[str, int] = {}  # tab -> panels that died on their own
        self.broken: set[str] = set()  # tabs given up on (CRASH_LIMIT), for this core's life
        self._mu = threading.Lock()
        self.started = False  # the start batch ran
        self._passes = 0  # reconcile passes so far
        self._restore_seen: int | None = None  # the pass that first saw a restore

    # -- state -----------------------------------------------------------------------------

    def _load(self) -> dict:
        try:
            data = json.loads(self.state_path.read_text()) if self.state_path else {}
        except (OSError, ValueError):
            data = {}
        return data if isinstance(data, dict) else {}

    def _save_closed(self) -> None:
        """Saves the closed tabs and the panel record together."""
        if not self.state_path:
            return
        try:
            self.state_path.parent.mkdir(parents=True, exist_ok=True)
            tmp = self.state_path.with_suffix(".tmp")
            tmp.write_text(json.dumps({"closed": sorted(self.closed), "panes": self.record}, sort_keys=True))
            tmp.replace(self.state_path)
        except OSError as exc:
            log.warning("could not save the panels' state: %s", exc)

    # -- after a herdr restart -------------------------------------------------------------

    def _restored(self, snap: dict) -> dict:
        """Close the recorded panels that herdr's session restore brought back as plain shells
        (the proof is in the module docstring); the snapshot without them. Judged only on the
        first ``RESTORE_PASSES`` passes after the restore is first seen; then given up on."""
        by_id = {p.get("pane_id"): p for p in snap.get("panes") or [] if isinstance(p, dict)}
        gone: set[str] = set()
        changed = False
        candidates = []
        for pane_id, rec in list(self.record.items()):
            p = by_id.get(pane_id)
            if p is None:
                del self.record[pane_id]  # the pane is gone (or a fresh server numbers panes anew)
                changed = True
                continue
            was, now = rec.get("terminal"), p.get("terminal_id")
            if was and now and was != now:  # a restore, or a live handoff (the panel still runs)
                candidates.append((pane_id, p))
        if candidates and self._restore_seen is None:
            self._restore_seen = self._passes
        passes_since = self._passes - self._restore_seen if self._restore_seen is not None else 0
        for pane_id, p in candidates:
            verdict = self._judge(pane_id, p, snap) if passes_since < RESTORE_PASSES else "late"
            if verdict == "again":
                continue
            del self.record[pane_id]
            changed = True
            if verdict == "close":
                gone.add(pane_id)
                self.created.pop(pane_id, None)
                self.connected_once.discard(pane_id)
            else:
                log.info("%s was a panel before herdr restarted; left alone (%s)", pane_id, verdict)
        if changed:
            self._save_closed()
        if not gone:
            return snap
        return dict(snap, panes=[p for p in snap.get("panes") or [] if not (isinstance(p, dict)
                                                                            and p.get("pane_id") in gone)])

    def _judge(self, pane_id: str, p: dict, snap: dict) -> str:
        """One recorded pane with a new terminal: "close" (closed now), "again" (herdr could
        not say yet), or why it is left alone for good. Any sign that the operator used the
        pane is final. A panel that still runs after a live handoff is "in use" here, and
        ``_remember`` records it again under its new terminal."""
        if p.get("label") != herdr.PANEL_LABEL:
            return "relabelled"
        if p.get("agent"):
            return "an agent"
        if p.get("focused") or snap.get("focused_pane_id") == pane_id or _tab_focus(snap, pane_id):
            return "focused"
        seen = self._focused_ever(pane_id)
        if seen is None:
            return "again"  # no live focus stream: never decide without it
        if seen:
            return "focused"
        plain, shell = herdr.pane_shell(pane_id)
        if plain is None:
            return "again"
        if not plain:
            return "in use"  # a command, an exec'ed program or the rc: never looked at again
        age, children = herdr.shell_state(shell)
        if children is not False:
            return "in use"  # a background job, or unknown
        if age is None or age > RESTORED_SECONDS:
            return "not a fresh restore"
        # the last look right before the close: herdr has no conditional close, so a command
        # typed in the few ms between this look and the close would still go with the pane
        if herdr.pane_shell(pane_id) != (True, shell):
            return "in use"
        if self._focused_ever(pane_id) is not False:  # a click during the looks above
            return "focused"
        err = herdr.close_pane(pane_id)
        if err:
            return f"could not close: {err}"
        log.warning("closed %s: a panel herdr restored as a plain shell; a new one is made", pane_id)
        return "close"

    def _focused_ever(self, pane_id: str) -> bool | None:
        """Whether this core's focus stream has seen the pane focused, ever (its whole log: a
        core started after a crash only saw events after the restore); None when there is no
        live stream to ask."""
        if self.focus_log is None or not getattr(self.focus_log, "live", False):
            return None
        return any(d.get("type") == "pane_focused" and d.get("pane_id") == pane_id
                   for d in self.focus_log.since(float("-inf")))

    def _remember(self, snap: dict, sv: Survey) -> None:
        """Record our running panels (pane id, tab, herdr's terminal id) for after a herdr
        restart. Only panes this core opened or adopted, while herdr shows them running our
        viewer."""
        by_id = {p.get("pane_id"): p for p in snap.get("panes") or [] if isinstance(p, dict)}
        running = {p for ps in sv.panels.values() for p in ps}
        record = dict(self.record)  # a gone pane was dropped by _restored, at the start of the pass
        for pane_id, tab in self.created.items():
            term = (by_id.get(pane_id) or {}).get("terminal_id")
            if pane_id in running and term:
                record[pane_id] = {"tab": tab, "terminal": term}
        if record != self.record:
            self.record = record
            self._save_closed()

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
        self._passes += 1
        snap = self._restored(snap)
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
        self._remember(snap, sv)
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
        """The cockpit off, or the home panorama quits: close every
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
            if self.record:  # a clean end: nothing is left for a restore to bring back
                self.record = {}
                self._save_closed()
        return n
