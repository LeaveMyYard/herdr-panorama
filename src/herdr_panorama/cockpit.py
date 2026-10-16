"""Cockpit (experimental): panorama as the agent list docked on the left of the selected
agent's own tab. See docs/DESIGN.md, "Cockpit".

The only pane that ever moves is panorama's own. Selecting an agent moves panorama's pane
into that agent's tab (``herdr pane move``), swaps it to the left of the agent's pane, sets
its width, and the view focuses the agent. Agents never leave their tabs, so closing a tab
closes what it shows, and losing panorama's pane is harmless: the next ``prefix+space`` or
start opens it again (single instance, found by content: ``home.candidates``). With the
cockpit off, ``home.ensure`` brings a docked panorama back to a home tab of its own.

No state of its own: where panorama is, is read from herdr every time.
"""

from __future__ import annotations

import logging
import threading
import time
from collections import deque

from . import config, herdr, home

log = logging.getLogger("herdr_panorama.cockpit")

DEFAULT_WIDTH = 46
# herdr draws a frame around panes: a pane's terminal is narrower than its layout rect, and
# by how much varies (measured on herdr 0.9.3 with stty inside throwaway panes: 1 to 3
# columns, depending on the pane's side and the session; the operator's docked panorama: 2).
# So the view measures its own (rect width minus the width it is drawn at, while docked) and
# sets ``pane_frame``; DEFAULT_FRAME is only the fallback before the first measurement.
DEFAULT_FRAME = 2
pane_frame: int | None = None


def enabled() -> bool:
    value = config.load().get("cockpit", False)
    return home.enabled() and (value is True or str(value).lower() in ("true", "1", "yes", "on"))


def width() -> int:
    try:
        w = int(config.load().get("cockpit_width", DEFAULT_WIDTH))
    except (TypeError, ValueError):
        w = DEFAULT_WIDTH
    return max(36, min(119, w))  # 36: the header and footer fit whole; under 120: the narrow layout


def _pane(snap: dict, pane_id: str) -> dict | None:
    return next((p for p in snap.get("panes") or [] if isinstance(p, dict) and p.get("pane_id") == pane_id), None)


def _leave(snap: dict, me: home.Home) -> str | None:
    """Before panorama leaves a home tab that still holds other panes, that tab loses the
    ``◆ panorama`` label (``home.release_tab``). Error or None."""
    return home.release_tab(snap, me.tab_id, me.pane_id)


def follow_setting() -> bool:
    """``cockpit_follow`` (default true, only with the cockpit on): re-dock wherever the
    operator lands on an orchestrator."""
    value = config.load().get("cockpit_follow", True)
    return value is not False and str(value).lower() not in ("false", "0", "no", "off")


def keep_focus_setting() -> bool:
    """``dock_keep_focus`` (default true): Enter or a click on another orchestrator in the docked
    panel docks panorama next to it and leaves the keyboard on panorama, so ↑/↓ and Enter browse
    the chats one after another; ``g`` goes into the agent. False: Enter focuses the agent too."""
    value = config.load().get("dock_keep_focus", True)
    return value is not False and str(value).lower() not in ("false", "0", "no", "off")


def follow_tabs() -> str:
    """``cockpit_follow_tabs``: "any" (the default, the operator's choice) also follows into
    tabs the operator split (panorama then takes ~46 columns of that layout); "single" only
    into tabs whose one pane is the orchestrator. An explicit Enter docks anywhere either way."""
    return "single" if str(config.load().get("cockpit_follow_tabs", "any")).lower() == "single" else "any"


# Bounded settle wait for late focus events after the follow swap. herdr delivers focus
# events ~50 ms (median) and up to ~100 ms after the action (a ~100 ms flush tick, measured on
# an isolated server), so this keeps a 3x margin. The view is told about the dock *before* it.
SYNC_SECONDS = 0.3
moving_until = 0.0  # monotonic: a move of panorama's pane is under way (the view holds its panel)


class FocusLog:
    """The focus events panorama's event stream has seen, stamped on arrival (monotonic).
    The follow path uses it to see whether the operator went somewhere between its last
    look at focus and the swap that would otherwise pull them back."""

    def __init__(self) -> None:
        self._events: deque = deque(maxlen=64)
        self._cond = threading.Condition()
        self.live = False  # the stream is connected and acknowledged: a switch would be seen

    def record(self, data: dict) -> None:
        with self._cond:
            self._events.append((time.monotonic(), data))
            self._cond.notify_all()

    def since(self, t0: float) -> list[dict]:
        with self._cond:
            return [d for t, d in self._events if t >= t0]

    def wait_for(self, t0: float, pred, timeout: float) -> bool:
        """Until an event since ``t0`` matches ``pred`` (True), or ``timeout`` (False)."""
        end = time.monotonic() + timeout
        with self._cond:
            while True:
                if any(pred(d) for t, d in self._events if t >= t0):
                    return True
                left = end - time.monotonic()
                if left <= 0:
                    return False
                self._cond.wait(left)


QUIET = frozenset({"already there", "the operator is in panorama", "not an orchestrator's pane",
                   "another workspace", "no focused pane", "a tab with other panes", "panorama is zoomed",
                   "the operator moved on", "another panorama instance"})  # follow's normal "no"s


def _went_elsewhere(events: list[dict], agent: dict) -> tuple[str, str] | None:
    """Where the operator went, from focus events that are not the agent's own pane or tab,
    the last one winning: ("pane", pane_id) or ("tab", tab_id); None if they stayed."""
    target = None
    for d in events:
        if d.get("type") == "pane_focused" and d.get("pane_id") and d.get("pane_id") != agent["pane_id"]:
            target = ("pane", d["pane_id"])
        elif d.get("type") == "tab_focused" and d.get("tab_id") and d.get("tab_id") != agent["tab_id"]:
            target = ("tab", d["tab_id"])
    return target


def _send_back(where: tuple[str, str]) -> None:
    kind, ident = where
    err = herdr.focus_pane(ident) if kind == "pane" else herdr.focus_tab(ident)  # tab: its active pane
    if err:
        log.warning("could not return focus to %s after the swap: %s", ident, err)


def _move_next_to(snap: dict, me: home.Home, t: dict, keep_focus_on: str | None,
                  focus_log: FocusLog | None = None, on_docked=None,
                  focus_me: bool = False) -> tuple[str | None, str | None]:
    """The one dock path (under home's lock): release a shared home tab, move panorama's own
    pane in with ``--no-focus``, put it on the agent's left, size it. ``keep_focus_on``: the
    follow path, where the operator is on that agent and must stay exactly there; it swaps
    only with ``focus_log`` (the live event stream), else panorama stays on the right.
    ``focus_me`` (an explicit dock with ``dock_keep_focus``): focus ends on panorama itself."""
    target = t["pane_id"]
    if keep_focus_on is not None:
        # follow: one more look right before anything changes. The decision's checks took a
        # few herdr calls; if the operator went back to panorama (or anywhere) meanwhile,
        # moving now would pull panorama out from under them.
        if (herdr.snapshot() or {}).get("focused_pane_id") != keep_focus_on:
            return None, "the operator moved on"
    err = _leave(snap, me)
    if err:
        return None, f"panorama stays: could not relabel the tab it would leave: {err}"
    global moving_until
    moving_until = time.monotonic() + 1.0  # herdr resizes this pane in steps until the ratio is set
    res, err = herdr.move_pane_to_tab(me.pane_id, t["tab_id"], target)
    if res is None:
        moving_until = 0.0
        return None, f"could not move panorama next to {target}: {err}"
    mine = (res.get("pane") or {}).get("pane_id") or me.pane_id  # a new id across workspaces
    home._save(home.Home(mine, t["tab_id"]))
    if on_docked is not None:
        on_docked(target)  # the view shows the new dock now, not after the swap and its settle wait
    # Moved in on the right; the list goes left with a swap. herdr's swap has no --no-focus:
    # it focuses its *source* pane, so the agent is the source and focus lands on the agent.
    # In the follow path the operator is already on that agent; if they have moved on in the
    # meantime, the swap is skipped (panorama stays on the right) rather than pull them back.
    if keep_focus_on is not None:
        if focus_log is None or not focus_log.live:  # a switch in the swap's window could go unseen
            fit(mine, t["tab_id"])
            return mine, None
        t0 = time.monotonic()
        now = herdr.snapshot() or {}
        if now.get("focused_pane_id") != keep_focus_on:
            fit(mine, t["tab_id"])
            return mine, None
    serr = herdr.swap_panes(mine, target) if focus_me else herdr.swap_panes(target, mine)
    if serr:
        log.warning("panorama stays right of %s: %s", target, serr)
    if focus_me:
        # dock_keep_focus: the client to the agent's tab, the keyboard on panorama there. The
        # swap's focus of its source is not enough: herdr logs that tab as focused, but an attached
        # client keeps showing the tab panorama left and puts focus back there on its next input
        # (the operator's report, 0.9.3: Enter seemed to close the panel; the open key then
        # "fixed" it). The open key's own sequence, a tab focus then a pane focus, moves it.
        ferr = herdr.focus_tab(t["tab_id"]) or herdr.focus_pane(mine)
        if ferr:
            log.warning("could not focus panorama after docking: %s", ferr)
    if keep_focus_on is not None and not serr:
        # Close the check-to-swap window. If the operator switched away just before the swap,
        # the swap pulled them back onto the agent, and herdr reports that as the agent's
        # pane_focused, after their own switch (events are in order): seeing it means the log
        # is complete. In the normal case the agent was already focused and the swap sends no
        # event at all, so this is a bounded settle wait for late events (SYNC_SECONDS).
        focus_log.wait_for(t0, lambda d: d.get("type") == "pane_focused" and d.get("pane_id") == target,
                           SYNC_SECONDS)
        elsewhere = _went_elsewhere(focus_log.since(t0), t)
        if elsewhere:
            t1 = time.monotonic()
            _send_back(elsewhere)
            later = _went_elsewhere(focus_log.since(t1), t)  # re-read once: they may have kept going
            if later and later != elsewhere:
                _send_back(later)
    fit(mine, t["tab_id"])
    return mine, None


def dock(target: str, me_id: str | None = None, on_docked=None,
         focus_me: bool = False) -> tuple[str | None, str | None]:
    t0 = time.perf_counter()
    try:
        return _dock(target, me_id, on_docked, focus_me)
    except home.LockBusy:
        return None, "another panorama operation holds the home lock"
    finally:
        herdr.trace("step dock", t0)


def _dock(target: str, me_id: str | None = None, on_docked=None,
          focus_me: bool = False) -> tuple[str | None, str | None]:
    """Move panorama's pane (``me_id``: the instance whose key was pressed, if verified)
    next to ``target`` (an agent's pane), on its left. Returns (panorama's pane id now,
    None), or (None, error) with panorama left where it was. The agent's pane never moves.
    ``focus_me``: focus ends on panorama (``dock_keep_focus``), not on the agent."""
    with home._lock():
        snap = herdr.snapshot()
        if snap is None:
            return None, "herdr did not answer"
        me = home.find(snap, prefer=me_id)
        if me is None:
            return None, "panorama's pane is not in herdr's snapshot"
        t = _pane(snap, target)
        if t is None or not t.get("agent"):
            return None, f"no agent in {target}"
        if t.get("tab_id") == me.tab_id:
            fit(me.pane_id, me.tab_id)
            return me.pane_id, None  # already next to it
        return _move_next_to(snap, me, t, None, None, on_docked, focus_me)


def follow(me_id: str | None, focus_log: FocusLog | None = None, on_docked=None) -> tuple[str | None, str]:
    t0 = time.perf_counter()
    out = (None, "error")
    try:
        out = _follow(me_id, focus_log, on_docked)
        return out
    except home.LockBusy:
        out = (None, "another panorama operation holds the home lock")
        return out
    finally:
        herdr.trace("step follow", t0, why=out[1])


def _follow(me_id: str | None, focus_log: FocusLog | None = None, on_docked=None) -> tuple[str | None, str]:
    """The operator landed somewhere (a tab or pane focus event, debounced): if that is a
    Claude orchestrator's pane in panorama's workspace and panorama is not already in its tab,
    re-dock there without touching focus. Returns (panorama's pane id after a move, why) or
    (None, why not). Decided on a fresh snapshot under home's lock."""
    with home._lock():
        known = home._load().get("pane_id")
        if me_id and known and known != me_id:
            return None, "another panorama instance"  # only the home instance follows
        snap = herdr.snapshot()
        if snap is None:
            return None, "herdr did not answer"
        me = home.find(snap, prefer=me_id)
        if me is None:
            return None, "panorama's pane is not verified"
        focused = snap.get("focused_pane_id")
        t = _pane(snap, focused) if focused else None
        mine = _pane(snap, me.pane_id) or {}
        if t is None:
            return None, "no focused pane"
        if focused == me.pane_id:
            return None, "the operator is in panorama"
        if t.get("agent") != "claude":
            return None, "not an orchestrator's pane"
        if t.get("workspace_id") != mine.get("workspace_id"):
            return None, "another workspace"
        if t.get("tab_id") == me.tab_id:
            return None, "already there"
        if follow_tabs() == "single":
            others = [p for p in snap.get("panes") or [] if isinstance(p, dict) and p.get("tab_id") == t.get("tab_id")
                      and p.get("pane_id") != focused]
            if others:
                return None, "a tab with other panes"  # cockpit_follow_tabs = "single"
        if (herdr.pane_layout(me.pane_id) or {}).get("zoomed"):
            return None, "panorama is zoomed"  # the operator is looking at the full graph
        moved, err = _move_next_to(snap, me, t, focused, focus_log, on_docked)
        return moved, err or "docked"


def undock(me: home.Home) -> home.Home | None:
    """Cockpit off: panorama's pane back to a tab of its own, labelled and first. Runs under
    home's lock (from ``ensure``). The new Home, or None (it stays where it is)."""
    # the workspace it is in, named explicitly (the new tab stays next to its agents)
    snap = herdr.snapshot() or {}
    ws = (_pane(snap, me.pane_id) or {}).get("workspace_id")
    if not ws:
        return None
    res, err = herdr.move_pane_new_tab(me.pane_id, ws, home.HOME_LABEL)
    if res is None:
        log.warning("could not move panorama back to its own tab: %s", err)
        return None
    pane = res.get("pane") or {}
    moved = home.Home(pane.get("pane_id") or me.pane_id, pane.get("tab_id") or "")
    if moved.tab_id:
        herdr.move_tab(moved.tab_id, 0)
    home._save(moved)
    return moved


# -- width ------------------------------------------------------------------------------

def _split_of(node, pane_id: str, path: list[bool]):
    """The split that holds ``pane_id`` as a direct child: (path, split node, pane is first)."""
    if not isinstance(node, dict) or node.get("type") != "split":
        return None
    for second, child in ((False, node.get("first")), (True, node.get("second"))):
        if isinstance(child, dict) and child.get("type") == "pane" and child.get("pane_id") == pane_id:
            return path, node, not second
    for second, child in ((False, node.get("first")), (True, node.get("second"))):
        found = _split_of(child, pane_id, path + [second])
        if found:
            return found
    return None


def _pane_ids(node) -> list:
    if not isinstance(node, dict):
        return []
    if node.get("type") == "pane":
        return [node.get("pane_id")]
    return _pane_ids(node.get("first")) + _pane_ids(node.get("second"))


def frame() -> int:
    return DEFAULT_FRAME if pane_frame is None else pane_frame


def fit(pane_id: str, tab_id: str, last_total: int | None = None) -> int | None:
    """Keep panorama's *drawable* width at ``cockpit_width`` columns in the split it shares
    with the agent: its rect is that plus herdr's pane frame.
    Returns that split's width; nothing is set when it equals ``last_total`` (so a split the
    operator dragged by hand stays until the tab's width changes)."""
    found = _split_of(herdr.layout_export(tab_id), pane_id, [])
    if not found:
        return None  # alone in its tab: no split to size
    path, node, first = found
    if node.get("direction") != "right":
        return None
    lay = herdr.pane_layout(pane_id) or {}
    rects = {p.get("pane_id"): p.get("rect") or {} for p in lay.get("panes") or [] if isinstance(p, dict)}
    mine = [rects[i] for i in _pane_ids(node) if i in rects]
    if not mine:
        return None
    total = max(r.get("x", 0) + r.get("width", 0) for r in mine) - min(r.get("x", 0) for r in mine)
    if total <= 0 or total == last_total:
        return total or None
    share = max(0.05, min(0.95, (width() + frame()) / total))
    herdr.set_split_ratio(tab_id, path, share if first else 1 - share)
    return total
