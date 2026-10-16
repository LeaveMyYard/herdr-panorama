"""Design 2a ("Graph · squares are subagents") as a pure cell grid.

A port of ``renderGraph`` from docs/design/swarm-min.js onto the real model:
same characters, colours and coordinates. ``render(swarm, view, w, h)`` returns
a ``Screen``; the Textual widget paints it, tests read ``Screen.text()``.

Navigation is identity-based: ``nav(swarm, view)`` lists the selectable ids of
each column in order, and the renderer highlights ``view.orch`` /
``view.thread`` / ``view.agent`` wherever they are.
"""

from __future__ import annotations

import math
import time
from dataclasses import dataclass, field

from .herdr import clean
from .model import (BLK, CXL, DONE, FAIL, GLYPH, INF, LABEL, LOST, Q, REV, RUN, SPIN, WAIT, Agent, Orchestrator,
                    Swarm, Thread, live, urgency)

# -- theme (TH.dark and the pod colours PB from the design) ---------------------------

BG, FG, HI, DIM, FAINT, LINE, SEL, HDR, KEY = ("#101215", "#c3c7cd", "#eceef1", "#80868f", "#4d535c", "#2c313a",
                                              "#1e2937", "#171a1f", "#e2cf9c")
ST = {Q: "#8d939d", RUN: "#68c2b5", REV: "#8eaee6", WAIT: "#ffb547", BLK: "#ffb547", DONE: "#6c9673",
      FAIL: "#ff6b6b", CXL: "#646a73", INF: "#8a9a8d", LOST: "#c08a8a"}
PB = {DONE: ("#2c6b4c", "#e3f2e9"), RUN: ("#1f625c", "#d8f2ee"), REV: ("#2e4f86", "#dfe8f8"),
      Q: ("#2a2e35", "#9aa0aa"), WAIT: ("#f2a93b", "#1b1203"), BLK: ("#f2a93b", "#1b1203"),
      FAIL: ("#d8363e", "#ffffff"), CXL: ("#24272c", "#6b717a"), "next": ("#1c1f24", "#454b54"),
      INF: ("#26312a", "#a9bcae"), LOST: ("#3a2526", "#e0b4b4")}  # muted: not confirmed (issue #2)

WIDE = 120
FILTERS = ("all", "running", "needs")
FILTER_LABEL = {"all": "all", "running": "running", "needs": "needs me"}
DONE_CARD = "done:"  # thread-column id of the "✓ n complete" card
NEW_ROW = "new:"  # orchestrator-column id of the "+ New agent" row (always last, never moves)
FIN_CARD = "fin:"  # agent-column id of the "n finished" card
CARDS = ("orch", "new", "thread", "done", "agent", "fin")  # click-target kinds that are whole cards


# -- text helpers (the design's fit / trunc / wrap / dur) ------------------------------

def trunc(s: str, n: int) -> str:
    s = str(s)
    return s if len(s) <= n else s[: max(0, n - 1)] + "…"


def fit(s: str, n: int) -> str:
    if n <= 0:
        return ""
    s = str(s)
    return s[: n - 1] + "…" if len(s) > n else s + " " * (n - len(s))


def wrap(text: str, w: int) -> list[str]:
    out, cur = [], ""
    for wd in str(text).split(" "):
        if not cur:
            cur = wd
        elif len(cur) + 1 + len(wd) <= w:
            cur += " " + wd
        else:
            out.append(cur)
            cur = wd
    if cur:
        out.append(cur)
    return [trunc(line, w) for line in out]


def dur(seconds: float | None) -> str:
    m = int(max(0.0, seconds or 0.0) // 60)
    return f"{m // 60}h{m % 60:02d}m" if m >= 60 else f"{m}m"


def hhmm(t: float | None) -> str:
    return time.strftime("%H:%M", time.localtime(t)) if t else ""


# -- the screen ------------------------------------------------------------------------

@dataclass
class Cell:
    ch: str = " "
    fg: str = FG
    bg: str | None = None
    bold: bool = False


class _Rows(list):
    """Rows created on first access: tall column canvases cost only what is drawn on them."""

    def __init__(self, w: int, h: int):
        super().__init__([None] * h)
        self.w = w

    def __getitem__(self, y):
        row = super().__getitem__(y)
        if row is None:
            row = [Cell() for _ in range(self.w)]
            super().__setitem__(y, row)
        return row

    def __iter__(self):
        return (self[y] for y in range(len(self)))


class Screen:
    def __init__(self, w: int, h: int):
        self.w, self.h = w, h
        self.c = _Rows(w, h) if h > 100 else [[Cell() for _ in range(w)] for _ in range(h)]
        # Click targets: (x0, x1, y, kind, identity). A click acts on the identity drawn there
        # at the last paint, never on a row index (the same rule as the cursor).
        self.targets: list[tuple[int, int, int, str, tuple]] = []
        # Wheel regions: (x0, x1, y0, y1, column) of each scrollable column as painted.
        self.columns: list[tuple[int, int, int, int, int]] = []
        self._anim: list | None = None  # animated(): the cells holding an Anim
        self._smooth = False

    def target(self, x0: int, x1: int, y: int, kind: str, ident: tuple) -> None:
        self.targets.append((x0, x1, y, kind, ident))

    def area(self, x0: int, x1: int, y0: int, y1: int, kind: str, ident: tuple) -> None:
        """A whole card (rows ``y0..y1``) as a click target. Register a card before the
        targets inside it (``target_at`` prefers the last one registered)."""
        for y in range(y0, y1):
            self.targets.append((x0, x1, y, kind, ident))

    def cells(self) -> dict[tuple[int, int], tuple[str, tuple]]:
        """What a click on each cell would hit, ``(x, y) -> (kind, identity)`` (the same
        precedence as ``target_at``: the last target registered wins)."""
        out: dict[tuple[int, int], tuple[str, tuple]] = {}
        for x0, x1, y, kind, ident in self.targets:
            for x in range(x0, x1):
                out[(x, y)] = (kind, ident)
        return out

    def card_at(self, x: int, y: int) -> tuple[str, tuple] | None:
        """The card (outermost target) at a cell, for the hover highlight."""
        for x0, x1, ty, kind, ident in self.targets:
            if ty == y and x0 <= x < x1 and kind in CARDS:
                return kind, ident
        return None

    def column_at(self, x: int, y: int) -> int | None:
        return next((c for x0, x1, y0, y1, c in self.columns if x0 <= x < x1 and y0 <= y < y1), None)

    def target_at(self, x: int, y: int) -> tuple[str, tuple] | None:
        for x0, x1, ty, kind, ident in reversed(self.targets):
            if ty == y and x0 <= x < x1:
                return kind, ident
        return None

    def drawn(self, y: int) -> bool:
        return not isinstance(self.c, _Rows) or list.__getitem__(self.c, y) is not None

    def put(self, x: int, y: int, s: str, fg: str | None = None, bg: str | None = None, bold: bool = False) -> int:
        for ch in (s.chars() if type(s) is Anim else str(s)):  # an Anim keeps moving, cell by cell
            if 0 <= y < self.h and 0 <= x < self.w:
                cell = self.c[y][x]
                cell.ch, cell.fg, cell.bold = ch, fg or FG, bool(bold)
                if bg is not None:
                    cell.bg = bg
            x += 1
        return x

    def P(self, x: int, y: int, segs) -> int:
        """Segments: plain strings or (text, fg[, bold[, bg]])."""
        for g in segs:
            if g is None:
                continue
            if isinstance(g, str):
                x = self.put(x, y, g, FG)
            else:
                text, fg = g[0], g[1]
                bold = g[2] if len(g) > 2 else False
                bg = g[3] if len(g) > 3 else None
                x = self.put(x, y, text, fg, bg, bold)
        return x

    def bg(self, x: int, y: int, w: int, h: int, col: str) -> None:
        for j in range(y, y + h):
            for i in range(x, x + w):
                if 0 <= j < self.h and 0 <= i < self.w:
                    self.c[j][i].bg = col

    def blit(self, src: "Screen", sy: int, dx: int, dy: int, h: int) -> None:
        """Copy rows ``sy..sy+h`` of ``src`` to ``(dx, dy)``."""
        for j in range(h):
            if not (0 <= sy + j < src.h and 0 <= dy + j < self.h) or not src.drawn(sy + j):
                continue
            for i in range(src.w):
                if 0 <= dx + i < self.w:
                    cell = src.c[sy + j][i]
                    if cell.ch != " " or cell.bg is not None:
                        self.c[dy + j][dx + i] = Cell(cell.ch, cell.fg, cell.bg, cell.bold)
        for x0, x1, ty, kind, ident in src.targets:
            if sy <= ty < sy + h and 0 <= dy + ty - sy < self.h:
                self.targets.append((dx + x0, dx + x1, dy + ty - sy, kind, ident))

    def text(self) -> str:
        return "\n".join("".join(c.ch for c in row).rstrip() for row in self.c)

    def animated(self) -> list[tuple[int, int, Cell]]:
        """The cells that move (an Anim glyph, fg or bg), found once."""
        if self._anim is None:
            self._anim = [(y, x, c) for y in range(self.h) if self.drawn(y) for x, c in enumerate(self.c[y])
                          if type(c.fg) is Anim or type(c.bg) is Anim or type(c.ch) is Anim]
            self._smooth = any(type(v) is Anim and not v.step for _, _, c in self._anim for v in (c.ch, c.fg, c.bg))
        return self._anim

    def smooth(self) -> bool:
        """Whether something moves between whole frames (a pulse or a blink): only then the
        colour tick runs between them."""
        self.animated()
        return self._smooth

    def runs(self, y: int) -> list[tuple[str, str, str | None, bool]]:
        out: list[list] = []
        for c in self.c[y]:
            if out and out[-1][1] == c.fg and out[-1][2] == c.bg and out[-1][3] == c.bold:
                out[-1][0] += c.ch
            else:
                out.append([c.ch, c.fg, c.bg, c.bold])
        return [tuple(r) for r in out]


# -- view state --------------------------------------------------------------------------

@dataclass
class View:
    orch: str | None = None  # pane_id
    thread: str | None = None  # thread id (or DONE_CARD + pane_id)
    agent: str | None = None  # agent id at any depth (or FIN_CARD + group key: thread id or parent agent id)
    focus: int = 0  # 0 orchestrators, 1 threads, 2 the thread's agents, 3 their subagents, ... (one per depth)
    filter: str = "all"
    done_open: set[str] = field(default_factory=set)  # pane ids whose done threads are unfolded
    fin_open: set[str] = field(default_factory=set)  # group keys whose finished agents are unfolded
    remember_thread: dict[str, str] = field(default_factory=dict)
    remember_agent: dict[str, str] = field(default_factory=dict)  # group key -> the child last selected in it
    offsets: dict[int, int] = field(default_factory=dict)  # per column scroll offset
    wheel: set[int] = field(default_factory=set)  # columns scrolled by the wheel: not following the selection
    hcol: int | None = None  # first column shown, set by a sideways scroll; None: follow the focus
    unseen: dict[str, str] = field(default_factory=dict)  # agent id -> its unseen event (unseen.py)
    hlo: int = 0  # the first column shown at the last paint (a sideways scroll starts from it)
    hover: tuple | None = None  # the (kind, identity) under the mouse
    dock: bool = False  # cockpit: panorama is docked next to an agent -> the 3a panel
    zoom_hint: bool = False  # docked next to an agent: z zooms between the panel and the graph
    now: float = 0.0
    # Animation time in design frames of 250 ms. Whole steps move spinners and sweeps; the colour
    # tick sets the fraction in between, which only colours follow (pulses, blinks: Anim).
    frame: float = 0


def _spin(frame: float, seq: int) -> str:
    return SPIN[(int(frame) + seq) % len(SPIN)]


def spin(frame: float, seq: int = 0) -> str:
    """A spinner glyph: one step per whole frame (an Anim, so frames need no re-render)."""
    return Anim(_spin, (seq,), frame, step=True)


def glyph(a: Agent, frame: float) -> str:
    return spin(frame, a.seq) if a.status == RUN else GLYPH.get(a.status, "?")


def nq(agents: list[Agent]) -> int:
    return sum(len(a.question_ids) for a in agents)


def needs(a: Agent) -> bool:
    return a.status in (WAIT, BLK, FAIL)


def orch_needs(o: Orchestrator) -> bool:
    return o.herdr_status == "blocked" or bool(o.inbox) or any(a.status == FAIL for a in o.agents())


def visible_orchs(swarm: Swarm, view: View) -> list[Orchestrator]:
    if view.filter == "running":
        return [o for o in swarm.orchestrators if any(not t.done for t in o.threads) or o.herdr_status == "working"]
    if view.filter == "needs":
        return [o for o in swarm.orchestrators if orch_needs(o)]
    return list(swarm.orchestrators)


def split_threads(o: Orchestrator, view: View) -> tuple[list[Thread], list[Thread]]:
    active = [t for t in o.threads if not t.done and t.status != "failed" or t.source == "session"]
    done = [t for t in o.threads if t not in active]
    failed = [t for t in done if t.status == "failed"]
    done = [t for t in done if t.status != "failed"]
    active = active + failed  # a failed thread stays visible (it needs the operator)
    if view.filter == "needs":
        active = [t for t in active if any(needs(a) for a in t.agents) or t.status == "failed"]
        done = []
    elif view.filter == "running":
        active = [t for t in active if t.status == "active"]
        done = []
    return active, done


def tree_kids(t: Thread) -> dict[str | None, list[Agent]]:
    """The thread's agents by the agent that spawned them (``None``: the thread's own). An agent
    whose parent is not in the thread, or that only a parent loop reaches, is the thread's own."""
    by_id = {a.id: a for a in t.agents}
    out: dict[str | None, list[Agent]] = {}
    for a in t.agents:
        p = a.parent if a.parent in by_id and a.parent != a.id else None
        out.setdefault(p, []).append(a)
    seen: set[str] = set()
    stack: list[str | None] = [None]
    while stack:
        for a in out.get(stack.pop(), []):
            if a.id not in seen:
                seen.add(a.id)
                stack.append(a.id)
    lost = [a for a in t.agents if a.id not in seen]
    if lost:
        out = {k: [a for a in v if a.id in seen] for k, v in out.items()}
        out.setdefault(None, []).extend(lost)
    return out


def split_group(ags: list[Agent], kids: dict, view: View) -> tuple[list[Agent], list[Agent]]:
    """One group of siblings: the cards shown (live, failed, or with such a descendant) and the
    finished ones folded into its "n finished" card."""
    memo: dict[tuple[str, str], bool] = {}

    def has(a: Agent, test, key: str, depth: int = 0) -> bool:
        k = (key, a.id)
        if k not in memo:
            memo[k] = False  # a loop is cut by tree_kids; this only guards the recursion
            memo[k] = test(a) or (depth < 64 and any(has(c, test, key, depth + 1) for c in kids.get(a.id, ())))
        return memo[k]

    shown = [a for a in ags if has(a, lambda x: live(x.status) or x.status == FAIL, "live")]
    fin = [a for a in ags if a not in shown]
    if view.filter == "needs":
        shown, fin = [a for a in shown if has(a, needs, "needs")], []
    elif view.filter == "running":
        fin = []
    shown.sort(key=lambda a: (a.started or 0.0, a.seq))  # stable: a status change never reorders the cards
    fin.sort(key=lambda a: (urgency(a.status), a.started or 0.0))
    return shown, fin


def split_agents(t: Thread, view: View) -> tuple[list[Agent], list[Agent]]:
    """The thread's own (depth 1) agents: shown and finished."""
    kids = tree_kids(t)
    return split_group(kids.get(None, []), kids, view)


@dataclass
class Entry:
    """One card of the agent tree, in depth-first order."""
    ident: str  # agent id, or FIN_CARD + key for a "n finished" card
    depth: int  # 1: the thread's own agents
    key: str  # the group it is in: the thread id or the id of the agent that spawned it
    agent: Agent | None = None
    fin: list[Agent] | None = None  # the folded agents of a "n finished" card
    parent: "Entry | None" = None


def tree_entries(t: Thread, view: View) -> list[Entry]:
    kids = tree_kids(t)
    out: list[Entry] = []

    def group(pid: str | None, key: str, depth: int, parent: Entry | None) -> None:
        if depth > 64:
            return
        shown, fin = split_group(kids.get(pid, []), kids, view)

        def card(a: Agent) -> None:
            e = Entry(a.id, depth, key, agent=a, parent=parent)
            out.append(e)
            group(a.id, a.id, depth + 1, e)

        for a in shown:
            card(a)
        if fin:
            out.append(Entry(FIN_CARD + key, depth, key, fin=fin, parent=parent))
            if key in view.fin_open:
                for a in fin:
                    card(a)

    group(None, t.id, 1, None)
    return out


def path_ids(t: Thread | None, aid: str | None) -> list[str]:
    """The selected agent and the agents that spawned it, outermost first."""
    if t is None or not aid:
        return []
    by_id = {a.id: a for a in t.agents}
    kids = tree_kids(t)
    parent = {a.id: p for p, ags in kids.items() for a in ags}
    out: list[str] = []
    cur: str | None = aid if aid in by_id else None
    while cur is not None and cur not in out:
        out.append(cur)
        cur = parent.get(cur)
    return out[::-1]


def agent_depth(t: Thread | None, ident: str | None) -> int:
    """The depth of an agent (or a "n finished" card) in the thread's tree; 1 if unknown."""
    if t is None or not ident:
        return 1
    if ident.startswith(FIN_CARD):
        key = ident[len(FIN_CARD):]
        return 1 if key == t.id else len(path_ids(t, key)) + 1
    return max(1, len(path_ids(t, ident)))


def parent_key(t: Thread, ident: str) -> str:
    """The group key of an agent or a "n finished" card: the thread id or the parent agent id."""
    if ident.startswith(FIN_CARD):
        return ident[len(FIN_CARD):]
    p = path_ids(t, ident)
    return p[-2] if len(p) > 1 else t.id


def nav(swarm: Swarm, view: View) -> list[list[str]]:
    """Selectable ids per column, in screen order."""
    orchs = visible_orchs(swarm, view)
    cols: list[list[str]] = [[o.pane_id for o in orchs] + [NEW_ROW], [], []]
    o = next((o for o in orchs if o.pane_id == view.orch), None)
    if o is None:
        return cols
    active, done = split_threads(o, view)
    cols[1] = [t.id for t in active]
    if done:
        cols[1].append(DONE_CARD + o.pane_id)
        if o.pane_id in view.done_open:
            cols[1] += [t.id for t in done]
    t = find_thread(o, view.thread)
    if t is not None:
        for e in tree_entries(t, view):
            while len(cols) < 2 + e.depth:
                cols.append([])
            cols[1 + e.depth].append(e.ident)
    return cols


def agent_ids(cols: list[list[str]]) -> list[str]:
    """Every selectable agent-tree id (all depth columns)."""
    return [i for c in cols[2:] for i in c]


def find_orch(swarm: Swarm, pane_id: str | None) -> Orchestrator | None:
    return next((o for o in swarm.orchestrators if o.pane_id == pane_id), None)


def find_thread(o: Orchestrator | None, tid: str | None) -> Thread | None:
    if o is None or not tid:
        return None
    return next((t for t in o.threads if t.id == tid), None)


def find_agent(t: Thread | None, aid: str | None) -> Agent | None:
    if t is None or not aid:
        return None
    return next((a for a in t.agents if a.id == aid), None)


# -- small pieces ------------------------------------------------------------------------

def pod(s: Screen, x: int, y: int, st: str, g: str, sel: bool = False) -> None:
    b, f = PB.get(st, PB[Q])
    if len(g) == 1:  # the glyph on its own: a spinner stays an Anim
        s.P(x, y, [(" ", f, True, b), (g, f, True, b), (" ", f, True, b)])
    else:
        s.P(x, y, [((" " + g)[:3], f, True, b)])
    if sel:
        s.put(x - 1, y, "[", HI, None, True)
        s.put(x + 3, y, "]", HI, None, True)


def apod(s: Screen, x: int, y: int, a: Agent, view: View, sel: bool = False) -> None:
    pod(s, x, y, a.status, glyph(a, view.frame), sel)


HOVER = FAINT  # the border of an unselected card under the mouse


def border(view: View, kind: str, ident: tuple, focus: bool, is_sel: bool) -> str:
    if focus:
        return HI
    if is_sel:
        return DIM
    return HOVER if view.hover == (kind, ident) else LINE


def rbox(s: Screen, x: int, y: int, w: int, h: int, col: str) -> None:
    s.put(x, y, "╭" + "─" * (w - 2) + "╮", col)
    for j in range(1, h - 1):
        s.put(x, y + j, "│", col)
        s.put(x + w - 1, y + j, "│", col)
    s.put(x, y + h - 1, "╰" + "─" * (w - 2) + "╯", col)


def tags(s: Screen, x: int, y: int, w: int, items: list[str | None]) -> None:
    xx = x + w - 2
    for t in reversed([t for t in items if t]):
        xx -= len(t) + 2
        s.P(xx, y, [(" " + t + " ", DIM)])
        xx -= 1


def marks_of(agents: list[Agent], blocked: bool = False, extra_q: int = 0) -> list[tuple]:
    out = []
    if any(a.status == FAIL for a in agents):
        out.append(("✗", ST[FAIL], True))
    if blocked or any(a.status == BLK for a in agents):
        out.append(("!", ST[BLK], True))
    q = nq(agents) + extra_q
    if q:
        out.append(("?" + str(q), ST[WAIT], True))
    return out


def marks_right(s: Screen, xr: int, y: int, mk: list[tuple], orch: str | None = None) -> int:
    """Marks right-aligned at ``xr``; with ``orch``, the ``?N`` mark is a click target that
    answers that orchestrator's questions."""
    w = sum(len(m[0]) + 1 for m in mk) - 1
    x = xr - w
    for m in mk:
        start = x
        x = s.P(x, y, [m]) + 1
        if orch and m[0].startswith("?"):
            s.target(start, x - 1, y, "answer-orch", (orch,))
    return max(0, w)


# Box-drawing merge (design turn 3): U=1 D=2 L=4 R=8, rounded corners, tees where a
# link meets a border. A cell that already holds a line glyph keeps its colour.
BX = {3: "│", 12: "─", 10: "╭", 9: "╰", 6: "╮", 5: "╯", 11: "├", 7: "┤", 14: "┬", 13: "┴", 15: "┼",
      1: "│", 2: "│", 4: "─", 8: "─"}
XB = {"│": 3, "─": 12, "╭": 10, "╰": 9, "╮": 6, "╯": 5, "┌": 10, "└": 9, "┐": 6, "┘": 5, "├": 11, "┤": 7,
      "┬": 14, "┴": 13, "┼": 15}


def jn(s: Screen, x: int, y: int, bits: int, col: str = LINE) -> None:
    if not (0 <= y < s.h and 0 <= x < s.w):
        return
    c = s.c[y][x]
    if c.ch == "┃":  # an animated card edge: a link joins it as ┨ (from the left) / ┠ (to the right)
        if bits == 4:
            c.ch = "┨"
        elif bits == 8:
            c.ch = "┠"
        return
    had = XB.get(c.ch)
    if had is not None:
        c.ch = BX[had | bits]
    else:
        c.ch, c.fg, c.bold = BX[bits], col, False


def link(s: Screen, fx: int, fy: int | None, tx: int, to_x: int, ys: list[int], top: int, bottom: int) -> None:
    """Solid link from the source card's right border (fx, fy) to a trunk at tx, then into the
    left border of each target card at to_x: the borders turn into tees (├ … ┤)."""
    ys = [y for y in ys if top <= y < bottom]
    if fy is None or not (top <= fy < bottom) or not ys:
        return
    jn(s, fx, fy, 8)
    for x in range(fx + 1, tx):
        jn(s, x, fy, 12)
    y0, y1 = min([fy, *ys]), max([fy, *ys])
    for y in range(y0, y1 + 1):
        jn(s, tx, y, (1 if y > y0 else 0) | (2 if y < y1 else 0) | (4 if y == fy else 0) | (8 if y in ys else 0))
    for y in ys:
        for x in range(tx + 1, to_x):
            jn(s, x, y, 12)
        jn(s, to_x, y, 4)


@dataclass
class OrchState:
    st: str
    glyph: str
    label: str
    working: bool = False

    @property
    def quiet(self) -> bool:
        return self.st in (Q, DONE, CXL, INF, LOST)

    @property
    def attention(self) -> bool:
        return self.st in (FAIL, BLK, WAIT)


QUIET = (Q, DONE, CXL, INF, LOST)


def state_of(agents: list[Agent], all_done: bool, frame: int, working_extra: bool = False,
             blocked_extra: bool = False, waiting_extra: bool = False) -> OrchState:
    """The design's stOf: attention > working > reviewing > idle; done when everything is."""
    has = lambda st: any(a.status == st for a in agents)  # noqa: E731
    runner = next((a for a in agents if a.status == RUN), None)
    working = runner is not None or working_extra
    if all_done and not working and not blocked_extra:
        return OrchState(DONE, "✓", "all done")
    for st, g, label, hit in ((FAIL, "✗", "failed", has(FAIL)), (BLK, "!", "blocked", has(BLK) or blocked_extra),
                              (WAIT, "?", "needs you", has(WAIT) or waiting_extra)):
        if hit:
            return OrchState(st, g, label + (" · working" if working else ""), working)
    if working:
        return OrchState(RUN, glyph(runner, frame) if runner else spin(frame), "working", True)
    if has(REV):
        return OrchState(REV, "»", "reviewing")
    return OrchState(Q, "○", "idle")


def orch_state(o: Orchestrator, frame: int) -> OrchState:
    """The design's orchSt. Extensions (declared in DESIGN): herdr reporting the main session
    ``working`` counts as working, ``blocked`` as blocked, and inbox questions no agent filed
    as needs you: the main session's own state."""
    return state_of(o.agents(), all(t.done for t in o.threads), frame, o.herdr_status == "working",
                    o.herdr_status == "blocked", bool(o.inbox))


def thread_state(t: Thread, frame: int) -> OrchState:
    return state_of(t.agents, t.done, frame)


def agent_state(a: Agent) -> OrchState:
    return OrchState(a.status, "", "", a.status == RUN)


def edge(cv: Screen, x: int, y: int, w: int, h: int, st: OrchState, frame: float, seed: int = 0) -> None:
    """The design's animated card edge: the left border in the state colour (blinking when it
    needs you: see ``blink``), and while agents work a 4-cell bar in the running colour
    sweeping the bottom border one cell per frame, shifted by ``seed * 7`` so cards do not
    move in lockstep."""
    if st.st in QUIET:
        return
    col = blink_col(ST[st.st], frame) if st.attention else ST[st.st]
    for j in range(1, h - 1):
        cv.put(x, y + j, "┃", col)
    if st.working and 0 <= y + h - 1 < cv.h:
        n = w - 2
        row = cv.c[y + h - 1]
        for i in range(n):  # every cell of the bottom border: the bar moves without a re-render
            if 0 <= x + 1 + i < cv.w:
                c = row[x + 1 + i]
                args = (i, n, seed, str(c.ch), str(c.fg))
                c.ch, c.fg, c.bold = Anim(_sweep_ch, args, frame, True), Anim(_sweep_fg, args, frame, True), False


def sweep_pos(frame: float, seed: int, n: int) -> int:
    """The first cell of the 4-cell bar on an ``n``-cell bottom border (may be off either end)."""
    return (int(frame) + seed * 7) % (n + 6) - 3


def _sweep_ch(frame: float, i: int, n: int, seed: int, ch: str, fg: str) -> str:
    return "━" if 0 <= i - sweep_pos(frame, seed, n) < 4 else ch


def _sweep_fg(frame: float, i: int, n: int, seed: int, ch: str, fg: str) -> str:
    return ST[RUN] if 0 <= i - sweep_pos(frame, seed, n) < 4 else fg


# -- smooth colour animation (design: smooth, swarm-smooth.js) ------------------------------
#
# Colours are functions of continuous time, not of whole frames: an ``Anim`` is the colour at
# ``frame`` (it is a str, so everything that draws or compares colours takes it as is) and
# remembers how to recompute itself. Between two 250 ms frames the view's colour tick
# (tui.COLOUR_SECONDS) restyles only the cells holding an Anim (``restyle``), so pulses and
# blinks run as smooth truecolor gradients without a re-render. At whole frames every colour
# is exactly the old stepped one, so the golden frames are unchanged.

class Anim(str):
    """A colour (or a glyph) that moves: ``fn(frame, *args)`` evaluated at ``frame``. ``step``:
    it only changes at whole frames (spinners, the sweep)."""

    def __new__(cls, fn, args: tuple, frame: float, step: bool = False):
        a = str.__new__(cls, fn(frame, *args))
        a.fn, a.args, a.step, a.k, a.f = fn, args, step, int(frame), frame
        return a

    def chars(self) -> list:
        """One Anim per character (``put`` gives each cell its own)."""
        if len(self) == 1:
            return [self]
        return [Anim(_nth, (self.fn, self.args, i), self.f, self.step) for i in range(len(self))]

    def at(self, frame: float) -> Anim:
        if self.step and int(frame) == self.k:
            return self
        return Anim(self.fn, self.args, frame, self.step)


def _nth(frame: float, fn, args: tuple, i: int) -> str:
    return fn(frame, *args)[i]


BLINK_PERIOD = 4  # frames: one second, as the design's floor(F / 2) % 2


def blink(frame: float) -> float:
    """The attention blink, 1 on .. 0 off: on for a frame, a sine ease off over the next, off
    for a frame, a sine ease back on (half on, half off, as before; whole frames hit 1 or 0)."""
    p = frame % BLINK_PERIOD
    if p <= 1:
        return 1.0
    if p < 2:
        return (1 + math.cos(math.pi * (p - 1))) / 2
    if p <= 3:
        return 0.0
    return (1 - math.cos(math.pi * (p - 3))) / 2


def _blink(frame: float, on: str) -> str:
    k = blink(frame)
    return on if k >= 1 else FAINT if k <= 0 else mix(FAINT, on, k)


def blink_col(on: str, frame: float) -> str:
    """``on`` blinking against the faint colour."""
    return Anim(_blink, (on,), frame)


# -- unseen: the slow pulse (design turn 6, swarm-unseen.js) ------------------------------

PULSE_PERIOD = 12  # frames of 250 ms: one slow pulse every 3 s
_URGENCY = {"fail": 0, "wait": 1, "done": 2}


def wave(frame: float) -> float:
    """0 .. 1 .. 0 over one pulse, a sine ease (continuous in ``frame``)."""
    return (1 - math.cos(2 * math.pi * (frame % PULSE_PERIOD) / PULSE_PERIOD)) / 2


def mix(a: str, b: str, k: float) -> str:
    """``a`` moved ``k`` of the way to ``b`` (hex colours; JS Math.round)."""
    pa = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{math.floor(x + (y - x) * k + 0.5):02x}" for x, y in zip(pa, pb, strict=True))


def mark_of(agents, unseen: dict[str, str], roll: bool = False) -> tuple[str, bool] | None:
    """The most urgent unseen event among ``agents`` and whether it is the quiet kind: on a
    thread or orchestrator card (``roll``) a finished agent pulses quietly."""
    evs = sorted((unseen[a.id] for a in agents if a.id in unseen), key=_URGENCY.__getitem__)
    return (evs[0], roll and evs[0] == "done") if evs else None


def _pulse(frame: float, ev: str, quiet: bool, base: str) -> str:
    k = 0.15 + 0.3 * wave(frame) if quiet else 0.35 + 0.65 * wave(frame)
    return mix(base, ST[{"fail": FAIL, "wait": WAIT, "done": DONE}[ev]], k)


def _tint(frame: float, ev: str, quiet: bool, base: str) -> str:
    k = 0.04 + 0.1 * wave(frame) if quiet else 0.08 + 0.22 * wave(frame)
    return mix(base, ST[{"fail": FAIL, "wait": WAIT, "done": DONE}[ev]], k)


def pulse_col(m: tuple[str, bool], base: str, frame: float) -> str:
    """Never fully off, so a still frame shows it too: full 35..100 %, quiet 15..45 % of the
    event colour over ``base``."""
    return Anim(_pulse, (m[0], m[1], base), frame)


def pulse_tint(m: tuple[str, bool], base: str, frame: float) -> str:
    """The docked panel's background tint: 8..30 % (quiet 4..14 %) of the event colour."""
    return Anim(_tint, (m[0], m[1], base), frame)


def restyle(scr: Screen, frame: float) -> list[tuple[int, int, int]]:
    """Recolour the animated cells of ``scr`` (drawn at the same whole frame) at ``frame``: the
    screen then equals a full render at ``frame``. Returns the changed spans (y, x0, x1)."""
    spans: list[tuple[int, int, int]] = []  # runs of changed cells, row by row (cells are in order)
    memo: dict[int, Anim] = {}  # cells drawn with one Anim (a whole pulsing border) share it

    def at(a: Anim) -> Anim:
        new = memo.get(id(a))
        if new is None:
            new = memo[id(a)] = a.at(frame)
        return new

    for y, x, c in scr.animated():
        changed = False
        if type(c.ch) is Anim:
            nc = at(c.ch)
            changed = nc != c.ch
            c.ch = nc
        if type(c.fg) is Anim:
            nf = at(c.fg)
            changed = changed or nf != c.fg
            c.fg = nf
        if type(c.bg) is Anim:
            nb = at(c.bg)
            changed = changed or nb != c.bg
            c.bg = nb
        if changed:
            if spans and spans[-1][0] == y and x - spans[-1][2] <= 3:  # a short gap: one span
                spans[-1] = (y, spans[-1][1], x + 1)
            else:
                spans.append((y, x, x + 1))
    return spans


def pulse_box(cv: Screen, x: int, y: int, w: int, h: int, m: tuple[str, bool] | None, frame: float) -> None:
    """The whole border (corners, sides, the animated edge and its sweep) in the pulse colour."""
    if m is None:
        return
    c = pulse_col(m, LINE, frame)
    for i in range(w):
        for yy in (y, y + h - 1):
            if 0 <= yy < cv.h and 0 <= x + i < cv.w:
                cv.c[yy][x + i].fg = c
    for j in range(1, h - 1):
        for xx in (x, x + w - 1):
            if 0 <= y + j < cv.h and 0 <= xx < cv.w:
                cv.c[y + j][xx].fg = c


def animated(swarm: Swarm, unseen: dict[str, str] | None = None) -> bool:
    """Whether anything on screen moves (spinners, working bars, blinking edges, the unseen
    pulse); otherwise animation frames need no repaint."""
    if unseen and any(a.id in unseen for o in swarm.orchestrators for a in o.agents()):
        return True
    for o in swarm.orchestrators:
        if o.herdr_status in ("working", "blocked") or o.inbox:
            return True
        if any(a.status in (RUN, FAIL, BLK, WAIT) for a in o.agents()):
            return True
    return False


def cur_idx(t: Thread) -> int | None:
    if not t.phases:
        return None
    for i, p in enumerate(t.phases):
        if p.state in ("now", "failed"):
            return i
    if all(p.state in ("done", "skipped") for p in t.phases):
        return len(t.phases) - 1
    return 0


def phase_glyph(t: Thread, i: int, view: View) -> tuple[str, str]:
    p = t.phases[i]
    if p.state == "done":
        return "✓", ST[DONE]
    if p.state == "skipped":
        return "-", ST[CXL]
    if p.state == "next":
        return "·", FAINT
    if p.state == "failed":
        return "✗", ST[FAIL]
    here = [a for a in t.agents if live(a.status) and a.phases and a.phases[-1] == i]
    if here:
        u = min(here, key=lambda a: urgency(a.status))
        return glyph(u, view.frame), ST.get(u.status, ST[RUN])
    return "⠋", ST[RUN]


def thread_glyph(t: Thread, view: View) -> tuple[str, str]:
    ci = cur_idx(t)
    if ci is not None:
        return phase_glyph(t, ci, view)
    if t.done:
        return "✓", ST[DONE]
    if t.status == "failed":
        return "✗", ST[FAIL]
    return "⠋", ST[RUN]


def phase_sub(t: Thread, view: View) -> str:
    ci = cur_idx(t)
    if ci is None:
        return t.status
    p = t.phases[ci]
    out = p.name
    if p.start and p.state == "now":
        out += " · " + dur(view.now - p.start)
    elif p.start and p.end:
        out += " · " + dur(p.end - p.start)
    if t.pct is not None:
        out += f" · {t.pct}%"
    return out


def step_line(s: Screen, x: int, y: int, t: Thread, i: int, w: int, view: View) -> None:
    p = t.phases[i]
    cur = p.state in ("now", "failed")
    g, gc = phase_glyph(t, i, view)
    if p.state in ("next", "skipped") or not p.start:
        when = ""
    elif p.end:
        when = f"{hhmm(p.start)}–{hhmm(p.end)}"
    else:
        when = f"since {hhmm(p.start)} · {dur(view.now - p.start)}"
    name_fg = HI if cur else FAINT if p.state in ("next", "skipped") else FG
    xx = s.P(x, y, [(g, gc, True), " ", (fit(p.name, 13), name_fg, cur), (fit(when, 20), FG if cur else FAINT)])
    if p.note and xx < x + w - 2:
        s.P(xx, y, [(trunc(clean(p.note), x + w - xx), DIM)])


def pods_row(s: Screen, x: int, y: int, t: Thread, view: View, max_n: int, sel_agent: str | None,
             orch: str | None = None, top_only: bool = False) -> None:
    """With ``orch``, each pod is a click target selecting its agent. ``top_only``: pods for the
    thread's own agents, then "+N nested" for the agents they spawned (the selected pod is the
    one on the selected agent's path)."""
    nested = 0
    ags = t.agents
    if top_only:
        ags = tree_kids(t).get(None, [])
        nested = len(t.agents) - len(ags)
        path = path_ids(t, sel_agent)
        sel_agent = path[0] if path else sel_agent
    ags = sorted(ags, key=lambda a: (urgency(a.status), a.started or 0.0))
    lv = [a for a in ags if live(a.status) or a.status == FAIL]
    fin = [a for a in ags if a not in lv]
    room = max(0, max_n - len(lv))
    show_f = fin if len(fin) <= room else fin[: max(0, room - 1)]
    shown = (lv + show_f)[:max_n]
    for a in shown:
        apod(s, x, y, a, view, sel_agent == a.id)
        if orch:
            s.target(x, x + 3, y, "agent", (orch, t.id, a.id))
        x += 4
    hidden = len(lv) + len(fin) - len(shown)
    if hidden:
        x = s.P(x, y, [(f"+{hidden} done" if len(lv) <= max_n else f"+{hidden}", FAINT)]) + 1
    if nested:
        s.P(x, y, [(f"+{nested} nested", FAINT)])


def role_label(a: Agent) -> str:
    return a.role or a.agent_type or "agent"


def agent_detail_lines(a: Agent, w: int) -> list[list]:
    out: list[list] = []
    out.append([("now  ", FAINT), (trunc(clean(a.activity or LABEL.get(a.status, "")), w - 5), FG)])
    for q in a.questions:
        for i, line in enumerate(wrap(clean(q), w - 2)[:4]):
            out.append([("  " if i else "? ", ST[WAIT], True), (line, ST[WAIT])])
    for p in a.pending:
        if p.startswith("wants "):
            out.append([("! ", ST[BLK], True), ("wants ", DIM), (trunc(clean(p[6:]), w - 8), ST[BLK], True)])
        else:
            for i, line in enumerate(wrap(clean(p), w - 2)[:4]):
                out.append([("  " if i else "! ", ST[BLK], True), (line, ST[BLK])])
    if a.error:
        out.append([("✗ ", ST[FAIL], True), (trunc(clean(a.error), w - 2), ST[FAIL])])
    if a.last_message and not a.questions and not a.error:
        for line in wrap("“" + clean(a.last_message) + "”", w)[:2]:
            out.append([(line, DIM)])
    return out


def actions(s: Screen, x: int, y: int, a: Agent, inbox_ok: bool, ident: tuple | None = None,
            room: int | None = None) -> None:
    """The action hints; an enabled ``a answer`` is also a click target for ``ident``
    (orchestrator pane id, thread id, agent id). ``room``: the width they must fit in (a narrow
    card closes the gaps, then says just "answer")."""
    can = (a.status == WAIT and inbox_ok and bool(a.question_ids)) or a.status == BLK
    label = "answer in pane" if a.status == BLK else "answer"
    gap = 3
    items = [("g", "open pane"), ("t", "transcript"), ("a", label)]
    width = lambda g: sum(len(k) + 1 + len(lab) for k, lab in items) + 2 * g  # noqa: E731
    if room is not None and width(gap) > room:
        gap = 2
        if width(gap) > room:
            label = "answer"
    xx = x
    for k, lab, on in (("g", "open pane", True), ("t", "transcript", True), ("a", label, can)):
        start = xx
        xx = s.P(xx, y, [(k, KEY if on else FAINT, True), (" " + lab, FG if on else FAINT)]) + gap
        if on and ident:
            s.target(start, xx - gap, y, {"g": "goto", "t": "transcript", "a": "answer"}[k], ident)


# -- header, legend, keys ----------------------------------------------------------------

def header(s: Screen, swarm: Swarm, view: View, stale: str | None = None) -> None:
    s.bg(0, 0, s.w, 1, HDR)
    x = s.P(1, 0, [("herdr", HI, True), (" panorama", DIM)]) + 4
    for k in FILTERS:
        on = view.filter == k
        start = x
        x = s.P(x, 0, [(FILTER_LABEL[k], HI if on else FAINT, on)]) + 3
        s.target(start, x - 3, 0, "filter", (k,))
    if stale:
        s.P(x, 0, [(" stale ", "#1b1203", True, ST[WAIT]), (" " + trunc(clean(stale), max(0, s.w - x - 40)), FAINT)])
    c = swarm.counts()
    segs = [(f"? {c['questions']}", ST[WAIT], True), "  ", (f"! {c['blocked']}", ST[BLK], True), "  ",
            (f"✗ {c['failed']}", ST[FAIL], True), "    ", (hhmm(view.now), DIM)]
    if s.w < 60:
        segs = segs[-1:]
    total = sum(len(g if isinstance(g, str) else g[0]) for g in segs)
    s.P(s.w - total - 1, 0, segs)


LEGEND = [(DONE, "✓", "done"), (INF, "✓?", "done?"), (RUN, "⠋", "running"), (REV, "»", "review"),
          (Q, "○", "queued"), (WAIT, "?", "question"), (BLK, "!", "blocked"), (FAIL, "✗", "failed"),
          (LOST, "~", "lost")]
SHORT = {"running": "run", "question": "ask", "blocked": "block", "failed": "fail", "queued": "queue",
         "review": "rev"}


def legend(s: Screen, y: int) -> None:
    x = 1
    for k, g, lab in LEGEND:
        if s.w >= WIDE:
            pod(s, x, y, k, g)
            x = s.P(x + 4, y, [(lab, FAINT)]) + 2
        else:
            x = s.P(x, y, [(g, ST[k], True), (" " + SHORT.get(lab, lab), FAINT)]) + (2 if s.w >= 80 else 1)


WIDE_KEYS = [("↑↓", "move"), ("→", "open"), ("←", "back"), ("enter", "details"), ("g", "go to pane"),
             ("a", "answer"), ("i", "inbox"), ("f", "filter"), ("?", "help")]
NARROW_KEYS = [("↑↓", "move"), ("→", "open"), ("g", "pane"), ("a", "answer"), ("f", "filter"), ("?", "help")]


def keys(s: Screen, items: list[tuple[str, str]], inbox_ok: bool) -> None:
    y, x = s.h - 1, 1
    for k, lab in items:
        if k == "i" and not inbox_ok:
            continue
        x = s.P(x, y, [(k, FG, True), (" " + lab, FAINT)]) + 3


# -- columns (rendered on tall canvases, then cropped around the selection) --------------

TALL = 400


@dataclass
class Column:
    canvas: Screen
    anchors: list[int]  # y of each card's first line (for links)
    sel: tuple[int, int] | None  # selected card's (top, bottom)
    sel_anchor: int | None


def scroll(view: View, col: int, c: Column, height: int, content_h: int) -> int:
    off = view.offsets.get(col, 0)
    if c.sel and col not in view.wheel:
        top, bottom = c.sel
        if bottom - off > height:
            off = bottom - height
        if top - off < 0:
            off = top
    off = max(0, min(off, max(0, content_h - height)))
    view.offsets[col] = off
    return off


def machine_divider(cv: Screen, x: int, y: int, w: int, label: str) -> None:
    """The operator's choice for orchestrators on another herdr machine: a faint divider row
    with the machine's label above its group; the cards themselves are unchanged."""
    text = f" {clean(label)} "
    cv.put(x, y, "─", LINE)
    cv.put(x + 1, y, trunc(text, max(0, w - 2)), FAINT)
    cv.put(x + 1 + len(trunc(text, max(0, w - 2))), y, "─" * max(0, w - 1 - len(trunc(text, max(0, w - 2)))), LINE)


def _group_start(o: Orchestrator, prev: list) -> str | None:
    """The machine label when ``o`` starts a new machine's group (local ones have none).
    Groups are keyed by the machine's id: two machines may share a label."""
    key = getattr(o, "machine_id", None) or getattr(o, "machine", None)
    starts = key is not None and key != prev[0]
    prev[0] = key
    return (o.machine or key) if starts else None


def orch_column(swarm: Swarm, view: View, w: int) -> tuple[Column, int]:
    cv = Screen(w, TALL)
    y, anchors, sel, sel_anchor = 0, [], None, None
    prev: list = [None]
    for o in visible_orchs(swarm, view):
        label = _group_start(o, prev)
        if label:
            machine_divider(cv, 0, y, w, label)
            y += 1
        is_sel = o.pane_id == view.orch
        focus = is_sel and view.focus == 0
        os_ = orch_state(o, view.frame)
        cv.area(0, w, y, y + 4, "orch", (o.pane_id,))
        rbox(cv, 0, y, w, 4, border(view, "orch", (o.pane_id,), focus, is_sel))
        edge(cv, 0, y, w, 4, os_, view.frame, 0)
        pulse_box(cv, 0, y, w, 4, mark_of(o.agents(), view.unseen, True), view.frame)
        pod(cv, 2, y + 1, os_.st, os_.glyph)
        attributed = {i for a in o.agents() for i in a.question_ids}
        loose = sum(1 for q in o.inbox if q not in attributed)
        mk = [m for m in marks_of(o.agents(), o.herdr_status == "blocked", loose)
              if not (m[0] == "✗" and os_.st == FAIL) and not (m[0] == "!" and os_.st == BLK)]
        mw = marks_right(cv, w - 2, y + 1, mk, o.pane_id)
        cv.P(6, y + 1, [(trunc(clean(o.tab_label), w - 9 - mw), HI if is_sel else DIM if os_.quiet else FG, True)])
        active = sum(1 for t in o.threads if not t.done and t.source != "session")
        n_live = sum(1 for a in o.agents() if live(a.status) and not a.id.startswith("main:"))
        label = trunc(os_.label, w - 8)
        rest = "" if os_.quiet else trunc(f" · {active}/{n_live}", max(0, w - 8 - len(label)))
        cv.P(6, y + 2, [(label, FAINT if os_.quiet else ST[os_.st], not os_.quiet), (rest, DIM)])
        tags(cv, 0, y + 3, w, [o.flavour])
        anchors.append(y + 1)
        if is_sel:
            sel, sel_anchor = (y, y + 4), y + 1
        y += 5
    is_new = view.orch == NEW_ROW
    cv.area(0, w, y, y + 3, "new", ())
    rbox(cv, 0, y, w, 3, border(view, "new", (), is_new and view.focus == 0, is_new))
    cv.P(2, y + 1, [("+", KEY, True), ("  New agent", HI if is_new else DIM, is_new)])
    cv.P(w - 3, y + 1, [("n", FAINT)])
    if is_new:
        sel = (y, y + 3)
    y += 4
    return Column(cv, anchors, sel, sel_anchor), y


def thread_card(cv: Screen, x: int, y: int, w: int, t: Thread, view: View, is_sel: bool, focus: bool,
                seed: int = 1, orch: str = "") -> int:
    hh = 6 + len(t.phases) if is_sel and t.phases else 5
    cv.area(x, x + w, y, y + hh, "thread", (orch, t.id))
    rbox(cv, x, y, w, hh, border(view, "thread", (orch, t.id), focus, is_sel))
    edge(cv, x, y, w, hh, thread_state(t, view.frame), view.frame, seed)
    pulse_box(cv, x, y, w, hh, mark_of(t.agents, view.unseen, True), view.frame)
    mw = marks_right(cv, x + w - 2, y + 1, marks_of(t.agents))
    g, gc = thread_glyph(t, view)
    cv.P(x + 2, y + 1, [(g, gc, True), " ", (trunc(clean(t.name), w - 7 - mw), HI if is_sel else FG, True)])
    cv.P(x + 4, y + 2, [(trunc(clean(phase_sub(t, view)), w - 6), DIM)])
    pods_row(cv, x + 4, y + 3, t, view, max(1, (w - 14) // 4), view.agent if is_sel else None, orch, top_only=True)
    if is_sel:
        for i in range(len(t.phases)):
            step_line(cv, x + 4, y + 5 + i, t, i, w - 6, view)
    age = dur(view.now - t.started) if t.started else "queued"
    tags(cv, x, y + hh - 1, w, [age, t.worktree])
    return hh


def thread_column(o: Orchestrator, view: View, w: int) -> tuple[Column, int]:
    cv = Screen(w, TALL)
    active, done = split_threads(o, view)
    y, anchors, sel, sel_anchor = 0, [], None, None
    for t in active:
        is_sel = t.id == view.thread
        hh = thread_card(cv, 0, y, w, t, view, is_sel, is_sel and view.focus == 1, len(anchors) + 1,
                         o.pane_id)
        anchors.append(y + 1)
        if is_sel:
            sel, sel_anchor = (y, y + hh), y + 1
        y += hh + 1
    if done:
        card = DONE_CARD + o.pane_id
        is_sel = view.thread == card
        cv.area(0, w, y, y + 3, "done", (o.pane_id,))
        rbox(cv, 0, y, w, 3, border(view, "done", (o.pane_id,), is_sel and view.focus == 1, is_sel))
        pulse_box(cv, 0, y, w, 3, mark_of([a for t in done for a in t.agents], view.unseen, True), view.frame)
        names = ", ".join(clean(t.name) for t in done)
        fold = "▾" if o.pane_id in view.done_open else "▸"
        cv.P(2, y + 1, [("✓", ST[DONE], True), (f" {len(done)} complete", FG), ("  " + trunc(names, w - 21), FAINT)])
        cv.put(w - 3, y + 1, fold, FAINT)
        anchors.append(y + 1)
        if is_sel:
            sel, sel_anchor = (y, y + 3), y + 1
        y += 4
        if o.pane_id in view.done_open:
            for t in done:
                is_sel = t.id == view.thread
                hh = thread_card(cv, 0, y, w, t, view, is_sel, is_sel and view.focus == 1, len(anchors) + 1,
                                 o.pane_id)
                anchors.append(y + 1)
                if is_sel:
                    sel, sel_anchor = (y, y + hh), y + 1
                y += hh + 1
    if not active and not done:
        cv.P(2, 0, [("no subagents in the last hours", FAINT)])
        y = 1
    return Column(cv, anchors, sel, sel_anchor), y


def fit_segs(segs: list, room: int) -> list:
    """Segments cut to ``room`` cells in all, ending in "…" when cut: a card's title never runs
    into what is right-aligned after it (its state label, "› n")."""
    full = "".join(g[0] for g in segs)
    if len(full) <= room:
        return segs
    cut = trunc(full, max(1, room))
    out, pos = [], 0
    for g in segs:
        piece = cut[pos:pos + len(g[0])]
        pos += len(g[0])
        if piece:
            out.append((piece,) + tuple(g[1:]))
    return out


def agent_card(cv: Screen, x: int, y: int, w: int, a: Agent, t: Thread, view: View, inbox_ok: bool,
               col: int, on_path: bool = False, kids_off: int = 0) -> int:
    """An agent's card in its depth column (``col``): three rows, or unfolded with its details
    and actions when it is the selected agent. ``kids_off``: its children's column is scrolled
    off to the right, so the card says "› n". Its height."""
    is_sel = a.id == view.agent
    focus = is_sel and view.focus == col
    ident = (view.orch or "", t.id, a.id)
    if not is_sel:
        cv.area(x, x + w, y, y + 3, "agent", ident)
        rbox(cv, x, y, w, 3, border(view, "agent", ident, False, on_path))
        edge(cv, x, y, w, 3, agent_state(a), view.frame, a.seq)
        pulse_box(cv, x, y, w, 3, mark_of([a], view.unseen), view.frame)
        apod(cv, x + 2, y + 1, a, view)
        right = f"› {kids_off}" if kids_off else ""
        title = [(clean(role_label(a)), HI if on_path else FG, True), (" " + short_id(a), FAINT),
                 (" · " + clean(a.model) if a.model else "", DIM)]
        x2 = cv.P(x + 6, y + 1, fit_segs(title, max(1, x + w - 2 - (len(right) + 1 if right else 0) - (x + 6))))
        room = max(0, x + w - 3 - x2 - 3 - (len(right) + 1 if right else 0))
        cv.P(x2 + 3, y + 1, [(trunc(clean(a.activity or LABEL.get(a.status, "")), room), FAINT)])
        if right:
            cv.P(x + w - 2 - len(right), y + 1, [(right, DIM, True)])
        tags(cv, x, y + 2, w, [age_of(a, view), f"{a.tool_calls} calls" if a.tool_calls else None])
        return 3
    lines = agent_detail_lines(a, w - 6)
    hh = 6 + len(lines)
    cv.area(x, x + w, y, y + hh, "agent", ident)
    rbox(cv, x, y, w, hh, border(view, "agent", ident, focus, True))
    edge(cv, x, y, w, hh, agent_state(a), view.frame, a.seq)
    pulse_box(cv, x, y, w, hh, mark_of([a], view.unseen), view.frame)
    apod(cv, x + 2, y + 1, a, view)
    st = LABEL.get(a.status, a.status) + (" " + dur(view.now - a.ended) + " ago" if a.ended and not live(a.status) else "")
    title = [(clean(role_label(a)), HI, True), (" " + short_id(a), FAINT), (" · " + clean(a.model) if a.model else "", DIM)]
    cv.P(x + 6, y + 1, fit_segs(title, max(1, x + w - 2 - len(st) - 1 - (x + 6))))  # never into the state
    cv.P(x + w - 2 - len(st), y + 1, [(st, ST.get(a.status, FG), True)])
    for i, line in enumerate(lines):
        cv.P(x + 4, y + 3 + i, line)
    actions(cv, x + 4, y + hh - 2, a, inbox_ok, (view.orch, t.id, a.id), room=w - 6)
    phase = t.phases[a.phases[-1]].name if a.phases and a.phases[-1] < len(t.phases) else None
    tags(cv, x, y + hh - 1, w, [age_of(a, view), f"{a.tool_calls} calls" if a.tool_calls else None, phase])
    return hh


def fin_card(cv: Screen, x: int, y: int, w: int, e: Entry, t: Thread, view: View, col: int) -> None:
    """A group's "n finished" card (the thread's own group keeps its old identity)."""
    fin = e.fin or []
    is_sel = view.agent == e.ident
    ident = (view.orch or "", t.id) if e.key == t.id else (view.orch or "", t.id, e.key)
    cv.area(x, x + w, y, y + 3, "fin", ident)
    rbox(cv, x, y, w, 3, border(view, "fin", ident, is_sel and view.focus == col, is_sel))
    pulse_box(cv, x, y, w, 3, mark_of(fin, view.unseen), view.frame)
    xx = x + 2
    for a in fin[:5]:
        apod(cv, xx, y + 1, a, view)
        xx += 4
    roles: dict[str, int] = {}
    for a in fin:
        roles[role_label(a)] = roles.get(role_label(a), 0) + 1
    summary = f"{len(fin)} finished · " + ", ".join(f"{r} ×{n}" if n > 1 else r for r, n in roles.items())
    cv.P(xx + 1, y + 1, [(trunc(summary, x + w - xx - 5), FAINT)])
    cv.put(x + w - 3, y + 1, "▾" if e.key in view.fin_open else "▸", FAINT)


@dataclass
class Placed:
    e: Entry
    y: int
    h: int


def place_tree(entries: list[Entry], height) -> tuple[list[Placed], list[tuple[Placed | None, int, list[int]]], int]:
    """Lay the tree out in its depth columns. Each column stacks its cards; a group of children
    starts level with the agent that spawned it (or below the cards already in its column),
    and an agent with children starts below the previous group's last child, so two groups'
    connector lines never share rows. Returns the cards, the groups (parent, depth, anchor
    rows) and the height."""
    cur: dict[int, int] = {}
    trunk_end: dict[int, int] = {}
    placed: dict[int, Placed] = {}
    groups: dict[tuple[int, str], list[int]] = {}
    has_kids = {id(e.parent) for e in entries if e.parent is not None}
    out: list[Placed] = []
    for e in entries:
        d = e.depth
        y0 = placed[id(e.parent)].y if e.parent is not None else 0
        y = max(cur.get(d, 0), y0)
        if id(e) in has_kids:
            y = max(y, trunk_end.get(d + 1, -2) + 1)
        p = Placed(e, y, height(e))
        placed[id(e)] = p
        out.append(p)
        cur[d] = y + p.h + 1
        groups.setdefault((id(e.parent), d), []).append(y + 1)
        trunk_end[d] = max(trunk_end.get(d, -1), y + 1)
    parents = {id(p.e): p for p in out}
    gl = [(parents.get(pid), d, ys) for (pid, d), ys in groups.items()]
    return out, gl, max(cur.values(), default=0)


def short_id(a: Agent) -> str:
    return "main" if a.id.startswith("main:") else a.id[:7]


def age_of(a: Agent, view: View) -> str | None:
    if not a.started:
        return None
    end = view.now if live(a.status) else (a.ended or view.now)
    return dur(end - a.started)


# -- the two layouts ---------------------------------------------------------------------

def render(swarm: Swarm, view: View, w: int, h: int, stale: str | None = None) -> Screen:
    """``stale``: why the model may be out of date (a failed refresh, unreadable records)."""
    s = Screen(w, h)
    s.bg(0, 0, w, h, BG)
    header(s, swarm, view, stale)
    sideways = False
    if not swarm.herdr_ok:
        s.P(2, 2, [("herdr is not reachable", ST[WAIT], True), ("  (retrying)", FAINT)])
    elif not swarm.orchestrators:
        s.P(2, 2, [("No Claude Code sessions in herdr", DIM)])
    elif view.dock:
        return dock_panel(swarm, view, w, h, stale)
    elif w >= WIDE:
        sideways = _wide(s, swarm, view)
    else:
        _narrow(s, swarm, view)
    legend(s, h - 3)
    ks = WIDE_KEYS if w >= WIDE else NARROW_KEYS
    if w >= WIDE and sideways:
        ks = ks[:3] + [("shift+wheel", "scroll")] + ks[3:]
    keys(s, ks + ([("z", "panel")] if view.zoom_hint else []), swarm.inbox_ok)
    return s


DW, DMIN = 44, 36  # a depth column's width; the last one shown may shrink to DMIN to fit


def window(widths: list[int], gaps: list[int], f: int, avail: int, hcol: int | None) -> tuple[int, int]:
    """The columns shown (first, last): the focused one, the one before it (its parents) and,
    when it fits, the one after it (its children); then as many more as fit, right first.
    ``hcol`` (a sideways scroll) fixes the first one instead."""
    n = len(widths)
    mins = [w if i < 2 else DMIN for i, w in enumerate(widths)]

    def span(lo: int, hi: int) -> int:
        return sum(widths[lo:hi]) + sum(gaps[lo + 1:hi + 1]) + mins[hi]

    if hcol is not None:
        lo = hi = max(0, min(hcol, n - 1))
    else:
        lo, hi = (0 if f <= 1 else f - 1), min(n - 1, max(f + 1, 2))
        while hi > f and span(lo, hi) > avail:
            hi -= 1
        while lo < f and span(lo, hi) > avail:
            lo += 1
    while hi + 1 < n and span(lo, hi + 1) <= avail:
        hi += 1
    if hcol is None:
        while lo > 0 and span(lo - 1, hi) <= avail:
            lo -= 1
    return lo, hi


def col_label(i: int) -> str:
    return "orchestrators" if i == 0 else "threads" if i == 1 else f"depth {i - 1}"


def _wide(s: Screen, swarm: Swarm, view: View) -> bool:
    """Columns: orchestrators | threads | depth 1 | depth 2 | ... (as deep as the selected
    thread's tree), scrolled sideways around the focus. True when some are off-screen."""
    w, h = s.w, s.h
    OW = 28
    TW = max(36, min(50, int((w - 36 - 7 - 2) * 50 / 115)))
    top, bottom = 3, h - 4
    height = bottom - top
    o = find_orch(swarm, view.orch)
    if o is not None and o not in visible_orchs(swarm, view):
        o = None
    t = find_thread(o, view.thread)
    entries = tree_entries(t, view) if t is not None else []
    depth = max([e.depth for e in entries], default=1)
    widths = [OW, TW] + [DW] * (depth if t is not None else 0)
    if o is None:
        widths = widths[:1]
    elif t is None:
        widths = widths[:2] + [DW]  # 2a's empty agent column
    gaps = [0, 6] + [7] * (len(widths) - 2)
    f = min(view.focus, len(widths) - 1)
    lo, hi = window(widths, gaps, f, w - 6, view.hcol)
    x0 = 2 if lo == 0 else 4  # scrolled: two more columns for the lines in from the left
    avail = w - 2 - x0
    if hi >= 2:  # columns keep their width; the last one shrinks to fit (down to DMIN), never stretches:
        # the space left on the right stays empty (operator: a stretched card's sweep runs too long)
        widths[hi] = min(DW, avail - (sum(widths[lo:hi]) + sum(gaps[lo + 1:hi + 1])))
    xs: dict[int, int] = {}
    x = x0
    for i in range(lo, hi + 1):
        if i > lo:
            x += gaps[i]
        xs[i] = x
        x += widths[i]
    view.hlo = lo
    if view.hcol is not None:
        view.hcol = lo
    path = path_ids(t, view.agent)
    # the line above the columns: what is scrolled off, left and right
    if lo > 0:
        names = [clean(o.tab_label) if o else "", clean(t.name) if t else ""]
        by_id = {a.id: a for a in t.agents} if t else {}
        names += [f"{role_label(by_id[i])} {short_id(by_id[i])}" for i in path if i in by_id]
        names = [trunc(n, 28) for n in names[:lo]]
        segs: list = [("‹ ", DIM, True)]
        for i, n in enumerate(names):
            segs += [(n, FG if i == len(names) - 1 else DIM), (" › " if i < len(names) - 1 else "", FAINT)]
        segs.append((f"   {lo} column{'s' if lo > 1 else ''} left", FAINT))
        s.P(2, 2, segs)
    if hi < len(widths) - 1:
        k = len(widths) - 1 - hi
        what = col_label(hi + 1) if k == 1 or hi + 1 < 2 else f"depth {hi}–{len(widths) - 2}"
        text = f"{what}  {k} column{'s' if k > 1 else ''} right "
        s.P(w - 2 - len(text) - 1, 2, [(text, FAINT), ("›", DIM, True)])
    # orchestrators
    orch_y = None
    if lo == 0:
        oc, oh = orch_column(swarm, view, OW)
        off = scroll(view, 0, oc, height, oh)
        s.blit(oc.canvas, off, xs[0], top, height)
        s.columns.append((xs[0], xs[0] + OW, top, bottom, 0))
        orch_y = oc.sel_anchor - off + top if oc.sel_anchor is not None else None
    if o is None:
        return lo > 0 or hi < len(widths) - 1
    # threads
    t_y = None
    if lo <= 1 <= hi:
        TX = xs[1]
        tc, th = thread_column(o, view, widths[1])
        toff = scroll(view, 1, tc, height, th)
        s.blit(tc.canvas, toff, TX, top, height)
        s.columns.append((TX, TX + widths[1], top, bottom, 1))
        anchors = [a - toff + top for a in tc.anchors]
        if lo == 0:
            link(s, xs[0] + OW - 1, orch_y, xs[0] + OW + 2, TX, anchors, top, bottom)
        elif anchors:
            link(s, 0, anchors[0], 1, TX, anchors, top, bottom)
        t_y = tc.sel_anchor - toff + top if tc.sel_anchor is not None else None
    if t is None:
        return lo > 0 or hi < len(widths) - 1
    # the agent tree: one canvas for all depth columns, scrolled together
    vis = lambda d: lo <= 1 + d <= hi  # noqa: E731
    by_id = {a.id: a for a in t.agents}
    kids = tree_kids(t)

    def card_h(e: Entry) -> int:
        if e.agent is not None and e.ident == view.agent and vis(e.depth):
            return 6 + len(agent_detail_lines(e.agent, widths[1 + e.depth] - 6))
        return 3

    placed, groups, end = place_tree(entries, card_h)
    cv = Screen(w, TALL)
    sel = None
    on_path = set(path[:-1])
    for p in placed:
        if not vis(p.e.depth) or p.y + p.h > TALL:
            continue
        col = 1 + p.e.depth
        cx, cw = xs[col], widths[col]
        if p.e.fin is not None:
            fin_card(cv, cx, p.y, cw, p.e, t, view, col)
        else:
            a = p.e.agent
            off_right = len(kids.get(a.id, [])) if kids.get(a.id) and not vis(p.e.depth + 1) else 0
            agent_card(cv, cx, p.y, cw, a, t, view, swarm.inbox_ok, col, a.id in on_path, off_right)
        if p.e.ident == view.agent:
            sel = (p.y, p.y + p.h)
    for parent, d, ys in groups:
        if not vis(d):
            continue
        cx = xs[1 + d]
        if parent is not None:
            if vis(d - 1):
                link(cv, xs[d] + widths[d] - 1, parent.y + 1, cx - 4, cx, ys, 0, TALL)
            else:
                link(cv, 0, parent.y + 1, 1, cx, ys, 0, TALL)
        elif not (lo <= 1 <= hi) and ys:
            link(cv, max(0, cx - 7), ys[0], max(1, cx - 4), cx, ys, 0, TALL)
    col = Column(cv, [], sel, None)
    aoff = scroll(view, 2, col, height, end)
    s.blit(cv, aoff, 0, top, height)
    for d in range(1, len(widths) - 1):
        if vis(d):
            s.columns.append((xs[1 + d], xs[1 + d] + widths[1 + d], top, bottom, 2))
    if vis(1) and t_y is not None:
        d1 = next((ys for parent, d, ys in groups if parent is None), [])
        link(s, xs[1] + widths[1] - 1, t_y, xs[1] + widths[1] + 3, xs[2], [a - aoff + top for a in d1], top, bottom)
    if end - aoff > height:
        first = next(d for d in range(1, len(widths) - 1) if vis(d))
        s.P(xs[1 + first] + 2, bottom, [("↓ ", FAINT), ("more below", FAINT)])
    return lo > 0 or hi < len(widths) - 1


def _narrow(s: Screen, swarm: Swarm, view: View) -> None:
    w, h = s.w, s.h
    cv = Screen(w, TALL)
    y, sel = 0, None
    prev: list = [None]
    for o in visible_orchs(swarm, view):
        label = _group_start(o, prev)
        if label:
            machine_divider(cv, 0, y, w, label)
            y += 1
        is_sel = o.pane_id == view.orch
        mk = marks_of(o.agents(), o.herdr_status == "blocked")
        active, done = split_threads(o, view)
        os_ = orch_state(o, view.frame)
        state_col = FAINT if os_.quiet else ST[os_.st]
        cv.area(0, w, y, y + 1, "orch", (o.pane_id,))
        if not is_sel:
            if view.hover == ("orch", (o.pane_id,)):
                cv.put(0, y, "▏", HOVER)
            pod(cv, 1, y, os_.st, os_.glyph)
            mw = marks_right(cv, w - 2, y, mk, o.pane_id)
            name = trunc(clean(o.tab_label), max(8, w - 26 - mw))
            cv.P(5, y, [(name, DIM if os_.quiet else FG), (trunc("  " + os_.label, max(0, w - 8 - len(name) - mw)), state_col)])
            y += 2
            continue
        pod(cv, 1, y, os_.st, os_.glyph)
        name = trunc(clean(o.tab_label), max(8, w - 40))
        cv.P(5, y, [(name, HI, True), ("  " + os_.label, state_col), (f"  {o.flavour}", FAINT)])
        if view.focus == 0:
            cv.put(0, y, "▌", HI)
            sel = (y, y + 1)
        top_y = y
        y += 2
        anchors = []
        cards = list(active) + ([None] if done else []) + (list(done) if o.pane_id in view.done_open else [])
        for t in cards:
            if t is None:
                card = DONE_CARD + o.pane_id
                cv.area(3, w - 1, y, y + 1, "done", (o.pane_id,))
                names = ", ".join(clean(d.name) for d in done)
                cv.P(5, y, [("✓", ST[DONE], True), (trunc(f" {len(done)} complete · " + names, w - 9), FAINT)])
                if view.thread == card:
                    cv.put(3, y, "▸", HI if view.focus == 1 else DIM)
                    if view.focus == 1:
                        sel = (y, y + 1)
                anchors.append(y)
                y += 1
                continue
            exp = t.id == view.thread
            a = find_agent(t, view.agent) if exp else None
            det = agent_detail_lines(a, w - 10) if a else []
            steps = len(t.phases)
            hh = (4 + 1 + steps + 1 + 1 + len(det) + 1 + 1) if exp and a else (4 + 1 + steps + 1) if exp else 4
            focus = exp and view.focus == 1
            cv.area(3, w - 1, y, y + hh, "thread", (o.pane_id, t.id))
            rbox(cv, 3, y, w - 4, hh, border(view, "thread", (o.pane_id, t.id), focus, exp))
            edge(cv, 3, y, w - 4, hh, thread_state(t, view.frame), view.frame, len(anchors) + 1)
            mw = marks_right(cv, w - 3, y + 1, marks_of(t.agents))
            g, gc = thread_glyph(t, view)
            cv.P(5, y + 1, [(g, gc, True), " ", (trunc(clean(t.name), w - 13 - mw), HI if exp else FG, True)])
            pods_row(cv, 7, y + 2, t, view, 6, view.agent if exp else None, o.pane_id)
            sub = trunc(clean(phase_sub(t, view)), w - 40)
            cv.P(w - 3 - len(sub), y + 2, [(sub, DIM)])
            if exp:
                r = y + 4
                for i in range(steps):
                    step_line(cv, 7, r, t, i, w - 12, view)
                    r += 1
                r += 1
                if a:
                    cv.area(4, w - 2, r, r + len(det) + 3, "agent", (o.pane_id, t.id, a.id))
                    apod(cv, 7, r, a, view)
                    cv.P(11, r, [(role_label(a), HI, True),
                                 (trunc(f" {short_id(a)}" + (f" · {a.model}" if a.model else "") +
                                        f" · {age_of(a, view) or '-'} · {a.tool_calls} calls", w - 16 - len(role_label(a))), DIM)])
                    r += 1
                    for line in det:
                        cv.P(7, r, line)
                        r += 1
                    r += 1
                    actions(cv, 7, r, a, swarm.inbox_ok, (o.pane_id, t.id, a.id))
                if view.focus >= 1:
                    sel = (y, y + hh)
            anchors.append(y + 1)
            y += hh
        if anchors:
            last = anchors[-1]
            for yy in range(top_y + 1, last + 1):
                jn(cv, 1, yy, 1 | (2 if yy < last else 0) | (8 if yy in anchors else 0))
            for ay in anchors:
                jn(cv, 2, ay, 12)
                if cv.c[ay][3].ch in ("│", "┃"):
                    jn(cv, 3, ay, 4)
        y += 1
    is_new = view.orch == NEW_ROW
    cv.area(0, w, y, y + 1, "new", ())
    if not is_new and view.hover == ("new", ()):
        cv.put(0, y, "▏", HOVER)
    cv.P(1, y, [(" + ", KEY, True), ("  New agent", HI if is_new else DIM, is_new), ("  n", FAINT)])
    if is_new:
        cv.put(0, y, "▌", HI if view.focus == 0 else DIM)
        sel = (y, y + 1)
    y += 1
    top, height = 2, h - 2 - 3
    col = Column(cv, [], sel, None)
    off = scroll(view, 0, col, height, y)
    s.blit(cv, off, 0, top, height)
    s.columns.append((0, w, top, top + height, 0))


# -- the docked cockpit panel (design turn 3, variant 3a "Compact list", with 9e's tiles) --
# A port of swarm-dock.js vA (docs/design/swarm-dock.js) onto the real model: same
# characters, colours and coordinates in a 46-column pane. The design shows no separate
# "selected" state; the keyboard cursor uses its hover look (background and a dim ›).

DOCK_HOV = "#171a1f"  # swarm-dock.js HOV
DOCK_LABEL = {DONE: "done", FAIL: "failed", BLK: "blocked", RUN: "working", REV: "working", WAIT: "waiting",
              Q: "idle"}
DOCK_KEYS = [("↑↓", " move "), ("ret", " open "), ("a", " answer "), ("n", " new "), ("pfx+spc", " back")]


@dataclass
class DockState:
    st: str
    label: str
    q: int
    glyph: str
    working: bool
    agents: list
    live: list
    active: list
    done: list


def _att(st: str) -> bool:
    return st in (WAIT, BLK, FAIL)


def _gcol(st: str, view: View) -> str:
    return blink_col(ST[st], view.frame) if _att(st) else ST.get(st, FG)


def _sub_agents(o: Orchestrator) -> list[Agent]:
    """The orchestrator's subagents (its own main session is the card itself)."""
    return [a for t in o.threads if t.source != "session" for a in t.agents if not a.id.startswith("main:")]


def _threads(o: Orchestrator) -> list[Thread]:
    return [t for t in o.threads if t.source != "session"]


def dock_state(o: Orchestrator, frame: int) -> DockState:
    """swarm-dock.js oState: done > failed > blocked > working > waiting > idle. Extensions,
    as in 2a: herdr reporting the main session working counts as working, blocked as
    blocked, and the session's open inbox questions are its ?N."""
    ags = _sub_agents(o)
    threads = _threads(o)
    has = lambda st: any(a.status == st for a in ags)  # noqa: E731
    runner = next((a for a in ags if a.status == RUN), None)
    q = len(o.inbox) or nq(ags)
    working = runner is not None or has(REV) or o.herdr_status == "working"
    if threads and all(t.done for t in threads) and not working and o.herdr_status != "blocked" and not q:
        st = DONE
    elif has(FAIL):
        st = FAIL
    elif has(BLK) or o.herdr_status == "blocked":
        st = BLK
    elif working:
        st = RUN if (runner or o.herdr_status == "working") else REV
    elif q:
        st = WAIT
    else:
        st = Q
    g = (glyph(runner, frame) if runner else spin(frame)) if st == RUN else GLYPH.get(st, "?")
    live_ags = [a for a in ags if live(a.status)]
    return DockState(st, DOCK_LABEL[st], q, g, working, ags, live_ags,
                     [t for t in threads if not t.done], [t for t in threads if t.done])


def _dock_stcol(d: DockState) -> str:
    return ST[d.st] if _att(d.st) else ST[RUN] if d.working else DIM


def _thread_glyph(t: Thread, frame: int) -> tuple[str, str]:
    """swarm-dock.js tState."""
    if t.done:
        return "✓", DONE
    live_ags = sorted((a for a in t.agents if live(a.status)), key=lambda a: urgency(a.status))
    if live_ags:
        return glyph(live_ags[0], frame), live_ags[0].status
    return "○", Q


def _seglen(segs) -> int:
    return sum(len(g if isinstance(g, str) else g[0]) for g in segs if g is not None)


def _rp(s: Screen, xr: int, y: int, segs) -> None:
    s.P(xr - _seglen(segs), y, segs)


# Design 9e (swarm-tiles.js, the operator's pick): 2a's coloured tiles in the docked panel. A tile
# is 2a's pod, " g " in the state's colours; a needs-you tile blinks like the bare glyphs do, to
# TILE_OFF in its off phase.
TILE_OFF = ("#2a2e35", "#4d535c")  # swarm-tiles.js OFF: (background, glyph)


def _tile_bg(frame: float, on: str, off: str, ev: str | None, quiet: bool) -> str:
    """A needs-you tile's background: blinking between ``on`` and ``off`` (``blink``), then the
    unseen pulse over it (as swarm-tiles.js: pulseTile over tileC)."""
    base = _blink_between(frame, on, off)
    return base if ev is None else _pulse(frame, ev, quiet, base)


def _blink_between(frame: float, on: str, off: str) -> str:
    k = blink(frame)
    return on if k >= 1 else off if k <= 0 else mix(off, on, k)


def _padded(frame: float, fn, args: tuple) -> str:
    return " " + fn(frame, *args) + " "


def dock_tile(st: str, g: str, view: View, m: tuple[str, bool] | None = None) -> tuple:
    """swarm-tiles.js tile (and pulseTile): `` g `` bold in the state's tile colours. A needs-you
    tile blinks to TILE_OFF (easing, see ``blink``). Unseen (``m``): its background pulses toward the
    event colour, never fully off (the operator, Q115)."""
    b, f = PB.get(st, PB[Q])
    if _att(st):
        bg = Anim(_tile_bg, (b, TILE_OFF[0], m[0] if m else None, m[1] if m else False), view.frame)
        f = Anim(_blink_between, (f, TILE_OFF[1]), view.frame)
    else:
        bg = pulse_col(m, b, view.frame) if m else b
    text = Anim(_padded, (g.fn, g.args), view.frame, True) if type(g) is Anim else f" {g} "
    return (text, f, True, bg)


def _thread_segs(d: DockState, view: View) -> list:
    """swarm-tiles.js line 2 (rowTiles): one tile per active thread, its most urgent agent's
    state; the done threads fold into one ✓n tile; a space between tiles."""
    segs = []
    for t in d.active:
        g, st = _thread_glyph(t, view.frame)
        segs.append(dock_tile(st, g, view))
    if d.done:
        segs.append(dock_tile(DONE, "✓" + (str(len(d.done)) if len(d.done) > 1 else ""), view))
    return _spaced(segs)


def _fit_thread_segs(segs: list, counts: str, room: int) -> list:
    """An entry's second line in ``room`` cells: the thread tiles, then the counts. When they do
    not all fit (many threads), the first ones then ``+n`` for the rest, so the counts always
    stay whole."""
    tiles = segs[::2]  # _spaced: tiles at the even places
    budget = room - len(counts)
    if _seglen(segs) <= budget:
        return segs + [(counts, FAINT)]
    k = len(tiles)
    while k > 0 and _seglen(_spaced(tiles[:k])) + len(f" +{len(tiles) - k}") > budget:
        k -= 1
    more = [(f"{' ' if k else ''}+{len(tiles) - k}", FAINT)]
    line = _spaced(tiles[:k]) + more + [(counts, FAINT)]
    return line if _seglen(line) <= room else [(trunc(counts.strip(), room), FAINT)]


def _spaced(segs: list) -> list:
    """One space between glyphs: some terminals (ghostty) draw ◐◑◒◓ wider than a cell (East Asian
    ambiguous width), and glyphs in adjacent cells overlapped (the operator's report)."""
    out: list = []
    for i, g in enumerate(segs):
        if i:
            out.append(" ")
        out.append(g)
    return out


def _agent_segs(agents: list[Agent], frame: int, n: int = 8) -> list:
    """swarm-dock.js agSegs: the live agents' glyphs, most urgent first (up to ``n``), a space
    between them, then `` +N✓`` for the finished ones."""
    lv = sorted((a for a in agents if live(a.status)), key=lambda a: urgency(a.status))
    segs = _spaced([(glyph(a, frame), ST.get(a.status, FG), _att(a.status)) for a in lv[:n]])
    fin = len(agents) - len(lv)
    if fin:
        segs.append((f" +{fin}✓", FAINT))
    return segs


def _spawn_rows(t: Thread) -> list[tuple[str, bool, Agent]]:
    """The tree rows under a thread in the docked panel: (prefix, last sibling, agent) for each
    agent with children that is live or has a live descendant, depth first."""
    kids = tree_kids(t)
    memo: dict[str, bool] = {}

    def busy(a: Agent, depth: int = 0) -> bool:
        if a.id not in memo:
            memo[a.id] = False
            memo[a.id] = live(a.status) or (depth < 64 and any(busy(k, depth + 1) for k in kids.get(a.id, ())))
        return memo[a.id]

    out: list[tuple[str, bool, Agent]] = []

    def rec(pid: str | None, pre: str, depth: int) -> None:
        if depth > 64:
            return
        sp = [a for a in kids.get(pid, []) if kids.get(a.id) and busy(a)]
        sp.sort(key=lambda a: (a.started or 0.0, a.seq))
        for i, a in enumerate(sp):
            last = i == len(sp) - 1
            out.append((pre, last, a))
            rec(a.id, pre + ("  " if last else "│ "), depth + 1)

    rec(None, "", 0)
    return out


def dock_row(cv: Screen, y: int, o: Orchestrator, view: View, docked: bool, cursor: bool, w: int = 46) -> int:
    """One orchestrator at row ``y`` (swarm-dock.js vA with swarm-tiles.js 9e's tiles): tab number,
    state tile, name, ?N and state; its threads' tiles and counts, or for the docked one only the
    counts and then its thread list, a tile per thread. Its height."""
    d = dock_state(o, view.frame)
    threads = (d.active + ([None] if d.done else [])) if docked else []
    items: list = []  # (thread or None for the done row, None) or (thread, (prefix, last, agent)) tree rows
    for t in threads:
        items.append((t, None))
        if t is not None:
            items += [(t, r) for r in _spawn_rows(t)]
    hov = not docked and (cursor or view.hover == ("orch", (o.pane_id,)))
    h = 2 + len(items)
    xr = w - 1  # right-aligned fields end at the real right edge (the design's 45 at 46 columns)
    cv.area(0, w, y, y + h, "orch", (o.pane_id,))
    m = mark_of(o.agents(), view.unseen, True)  # unseen: the entry's two lines pulse as a tint
    base = SEL if docked else DOCK_HOV if hov else BG
    if docked or hov or m:
        cv.bg(0, y, w, 2, pulse_tint(m, base, view.frame) if m else base)
    if docked:
        for j in range(2 + len(items)):
            cv.put(0, y + j, "▌", KEY)
    elif hov:
        cv.put(0, y, "›", DIM)
    right = [("?" + str(d.q), ST[WAIT], True) if d.q else None, " " if d.q else None, (d.label, _dock_stcol(d))]
    tab = str(o.tab_no or "?")
    state = dock_tile(d.st, d.glyph, view)
    x_name = 2 + len(tab) + 1 + len(state[0]) + 1
    room = max(1, min(22, xr - _seglen(right) - 1 - x_name))  # a narrow pane cuts the name, never the state
    cv.P(2, y, [(tab, FAINT), " ", state, " ",
                (trunc(clean(o.tab_label), room), HI if docked or hov else DIM if d.st == DONE else FG, docked)])
    _rp(cv, xr, y, right)
    if d.q:
        xq = xr - _seglen(right)
        cv.target(xq, xq + 1 + len(str(d.q)), y, "answer-orch", (o.pane_id,))
    counts = f"{len(d.active)} active · {len(d.live)} agents"
    if items:  # 9e: the docked entry's list shows the thread tiles, so its line 2 keeps only the counts
        cv.P(x_name, y + 1, [(trunc(counts, w - x_name), FAINT)])
    else:
        cv.P(x_name, y + 1, _fit_thread_segs(_thread_segs(d, view), "  " + counts, w - x_name))
    last_thread = max((i for i, (_, r) in enumerate(items) if r is None), default=-1)
    for i, (t, row) in enumerate(items):
        yy = y + 2 + i
        if row is not None:
            # the agents of this thread that spawned others, indented by depth, each with its
            # own children's glyphs on the right (leaf agents stay glyphs)
            pre, last, a = row
            if i < last_thread:
                cv.put(3, yy, "│", LINE)
            x = cv.P(5, yy, [(pre + ("└" if last else "├"), LINE)]) + 1
            segs = _agent_segs(tree_kids(t).get(a.id, []), view.frame, 5)
            room = max(1, xr - _seglen(segs) - 1 - (x + len(glyph(a, view.frame)) + 1))
            cv.P(x, yy, [(glyph(a, view.frame), _gcol(a.status, view), _att(a.status)), " ",
                         (trunc(clean(f"{role_label(a)} {short_id(a)}"), room), FG)])
            _rp(cv, xr, yy, segs)
            continue
        cv.put(3, yy, "└" if i == last_thread else "├", LINE)
        if t is None:
            fm = mark_of([a for x in d.done for a in x.agents], view.unseen, True)  # unseen: the tile pulses
            cv.P(5, yy, [dock_tile(DONE, "✓", view, fm), " ", (f"{len(d.done)} done", FAINT)])
            continue
        g, st = _thread_glyph(t, view.frame)
        tile = dock_tile(st, g, view, mark_of(t.agents, view.unseen, True))  # unseen: its background pulses
        ags = _agent_segs(tree_kids(t).get(None, []), view.frame, 4)
        room = max(1, min(22, xr - _seglen(ags) - 1 - (5 + len(tile[0]) + 1)))
        cv.P(5, yy, [tile, " ", (trunc(clean(t.name), room), FG)])
        _rp(cv, xr, yy, ags)
    return h


def dock_panel(swarm: Swarm, view: View, w: int, h: int, stale: str | None = None) -> Screen:
    """The cockpit's docked panel (3a): header, one entry per orchestrator, + New agent,
    footer. Every entry is a click target for its orchestrator; ?N answers it."""
    s = Screen(w, h)
    s.bg(0, 0, w, h, BG)
    c = swarm.counts()
    orchs = visible_orchs(swarm, view)
    s.P(1, 0, [("SWARM", DIM, True), (f"  {len(orchs)} orchestrators", FAINT)])
    if stale:
        s.P(1 + 5 + 2 + len(f"{len(orchs)} orchestrators") + 1, 0, [(" stale ", "#1b1203", True, ST[WAIT])])
    _rp(s, w - 1, 0, [(f"?{c['questions']}", ST[WAIT], True), " ", (f"!{c['blocked']}", ST[BLK], True), " ",
                      (f"✗{c['failed']}", ST[FAIL], True)])
    s.put(0, h - 2, "─" * w, LINE)
    if view.zoom_hint:  # extension: the design's footer has no room for it, so it sits in the rule
        _rp(s, w - 2, h - 2, [(" ", LINE), ("z", KEY), (" graph ", FAINT)])
    keys_ = [(k, lab) for k, lab in DOCK_KEYS if k != "a" or swarm.inbox_ok]
    for drop in ("n", "a", "ret", "↑↓"):  # a narrow pane drops hints whole, never cuts one
        if 1 + sum(len(k) + len(lab) for k, lab in keys_) <= w:
            break
        keys_ = [kl for kl in keys_ if kl[0] != drop]
    x = 1
    for k, lab in keys_:
        x = s.P(x, h - 1, [(k, KEY), (lab, FAINT)])
    cv = Screen(w, TALL)
    y, sel = 0, None
    prev: list = [None]
    for o in orchs:
        label = _group_start(o, prev)
        if label:
            machine_divider(cv, 0, y, w, label)
            y += 1
        cursor = view.orch == o.pane_id
        ch = dock_row(cv, y, o, view, o.pane_id == swarm.docked, cursor, w)
        if cursor:
            sel = (y, y + ch)
        y += ch + 1
    hov = view.orch == NEW_ROW or view.hover == ("new", ())
    cv.area(1, w - 1, y, y + 1, "new", ())
    if hov:
        cv.bg(1, y, w - 2, 1, DOCK_HOV)
    cv.P(2, y, [("+", KEY, True), " ", ("New agent", HI if hov else DIM)])
    _rp(cv, w - 1, y, [("n", FAINT)])
    if view.orch == NEW_ROW:
        sel = (y, y + 1)
    y += 1
    top, height = 2, h - 2 - 2
    col = Column(cv, [], sel, None)
    off = scroll(view, 0, col, height, y)
    s.blit(cv, off, 0, top, height)
    s.columns.append((0, w, top, top + height, 0))
    return s
