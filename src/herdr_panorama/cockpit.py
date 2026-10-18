"""Cockpit: per-tab panels, each orchestrator tab its own panorama panel on the left of its
agent, drawn from the home panorama's state (docs/DESIGN.md, "Per-tab panels"). The panels
are opened and placed by panels.py; this module holds the cockpit's settings, the focus log the
start batch reads, the width fit, and ``undock`` (a home panorama left docked next to an agent
goes back to a tab of its own).

The moving cockpit (one panorama pane moved into the selected agent's tab, following the
operator) is gone (operator decision Q187, 2026-10-18): an old ``cockpit_panels = "moving"``
reads as per-tab, and ``cockpit_follow`` is no longer read.

No state of its own: where panes are, is read from herdr every time.
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
# The moving cockpit measured its own; the per-tab panels use the operator's measured 2.
DEFAULT_FRAME = 2


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


def keep_focus_setting() -> bool:
    """``dock_keep_focus`` (default true): Enter or a click on another orchestrator in a panel
    switches to its tab's panel and leaves the keyboard on it, so ↑/↓ and Enter browse the
    chats one after another; ``g`` goes into the agent. False: Enter focuses the agent too."""
    value = config.load().get("dock_keep_focus", True)
    return value is not False and str(value).lower() not in ("false", "0", "no", "off")


def per_tab() -> bool:
    """The cockpit is on: per-tab panels, the home panorama stays home and feeds them
    (panels.py). ``cockpit_panels`` is not read any more: per-tab is the only kind (Q187)."""
    return enabled()


def arrows_setting() -> bool:
    """``dock_on_arrows`` (default true, the operator's proposal): in a panel ↑/↓ switch the
    chats themselves, once the arrows settle, the keyboard staying on the panels; Enter goes
    into that agent. False: Enter switches (and keeps the keyboard with ``dock_keep_focus``).
    Needs ``dock_keep_focus``: an arrow never moves the keyboard into an agent."""
    value = config.load().get("dock_on_arrows", True)
    on = value is not False and str(value).lower() not in ("false", "0", "no", "off")
    return on and keep_focus_setting()


def follow_tabs() -> str:
    """``cockpit_follow_tabs``: "any" (the default, the operator's choice) gives a panel to
    tabs the operator split too; "single" only to tabs whose one pane is the orchestrator."""
    return "single" if str(config.load().get("cockpit_follow_tabs", "any")).lower() == "single" else "any"


# Bounded settle wait for late focus events after a swap. herdr delivers focus events ~50 ms
# (median) and up to ~100 ms after the action (a ~100 ms flush tick, measured on an isolated
# server), so this keeps a 3x margin.
SYNC_SECONDS = 0.3


class FocusLog:
    """The focus events panorama's event stream has seen, stamped on arrival (monotonic).
    The per-tab panels' start batch uses it to see whether the operator went somewhere while
    it swapped panels into place (panels.py's restore guard)."""

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


def undock(me: home.Home) -> home.Home | None:
    """A home panorama found docked next to an agent (left by the moving cockpit of an older
    version): panorama's pane back to a tab of its own, labelled and first. Runs under
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
    return DEFAULT_FRAME


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
