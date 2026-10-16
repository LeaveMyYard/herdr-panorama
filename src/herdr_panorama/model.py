"""The derived swarm model: orchestrators -> threads -> phases / agents.

Plain dataclasses; ``to_json`` gives what ``swarm snapshot --json`` prints.
Times are POSIX seconds (float) or None.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field

# Agent status codes (the design's vocabulary).
Q, RUN, REV, WAIT, BLK, FAIL, DONE, CXL = "q", "run", "rev", "wait", "blk", "fail", "done", "cxl"
# INF: the ending was inferred from silence, not recorded (issue #2); LOST: the hook recorded a
# start, no stop, and the Claude process that ran the agent is gone.
INF, LOST = "inf", "lost"
GLYPH = {Q: "○", RUN: "⠋", REV: "»", WAIT: "?", BLK: "!", DONE: "✓", FAIL: "✗", CXL: "-", INF: "✓?", LOST: "~"}
LABEL = {Q: "queued", RUN: "running", REV: "reviewing", WAIT: "needs answer", BLK: "blocked",
         DONE: "done", FAIL: "failed", CXL: "cancelled", INF: "done?", LOST: "lost"}
ORDER = [FAIL, BLK, WAIT, REV, RUN, Q, LOST, INF, DONE, CXL]  # most urgent first
# The running spinner: a braille arc turning a quarter per frame (top-left, top-right, bottom-right,
# bottom-left), the old ◐◓◑◒ rhythm. Braille is drawn cell-exact by terminals (ghostty's own sprites);
# the half circles came from a fallback font, wider than a cell, and overlapped (the operator's report).
# This table and GLYPH are the one place glyphs are chosen (the design's swarm-tui.js SPIN / GL match).
SPIN = "⠋⠹⠼⠧"


def live(status: str) -> bool:
    return status not in (DONE, CXL, INF, LOST)


def urgency(status: str) -> int:
    return ORDER.index(status) if status in ORDER else len(ORDER)


@dataclass
class Phase:
    name: str
    state: str  # done | now | next | failed | skipped
    start: float | None = None
    end: float | None = None
    note: str | None = None


@dataclass
class Agent:
    id: str
    description: str = ""
    role: str = "agent"
    agent_type: str | None = None
    model: str | None = None
    status: str = RUN
    tool_use_id: str | None = None
    started: float | None = None
    ended: float | None = None
    last_at: float | None = None
    tool_calls: int = 0
    activity: str = ""
    last_message: str = ""
    questions: list[str] = field(default_factory=list)  # open inbox question texts
    pending: list[str] = field(default_factory=list)  # blocking prompt: AskUserQuestion texts or "wants Tool(...)"
    question_ids: list[int] = field(default_factory=list)
    error: str | None = None
    phases: list[int] = field(default_factory=list)
    parent: str | None = None
    depth: int = 1
    resumes: int = 0
    inferred: bool = False  # status guessed from the agent's own file
    transcript: str | None = None
    seq: int = 0  # stable index for the spinner frame


@dataclass
class Thread:
    id: str
    name: str
    source: str  # explicit | workflow | ref | turn | session
    phases: list[Phase] = field(default_factory=list)
    agents: list[Agent] = field(default_factory=list)
    status: str = "active"  # active | done | failed | cancelled
    started: float | None = None
    ended: float | None = None
    worktree: str | None = None
    pct: int | None = None
    flavour: str | None = None

    @property
    def done(self) -> bool:
        return self.status in ("done", "cancelled")


@dataclass
class Orchestrator:
    pane_id: str
    tab_id: str | None
    tab_label: str
    session_id: str | None
    herdr_status: str = "unknown"
    workspace_id: str | None = None
    focused: bool = False
    cwd: str | None = None
    flavour: str = "agents"
    transcript: str | None = None
    pending: list[str] = field(default_factory=list)  # blocked: question texts or "wants Bash(...)"
    inbox: list[int] = field(default_factory=list)  # open inbox question ids of this session
    inbox_texts: list[str] = field(default_factory=list)
    threads: list[Thread] = field(default_factory=list)
    hidden_done: int = 0  # done threads older than the window
    tab_no: int | None = None  # the tab's position in its workspace, 1-based (herdr's tab number)
    machine: str | None = None  # a saved herdr SSH machine's label; None = this machine
    machine_id: str | None = None  # that machine's stable id (identity, --machine)

    def agents(self) -> list[Agent]:
        return [a for t in self.threads for a in t.agents]


@dataclass
class Swarm:
    orchestrators: list[Orchestrator] = field(default_factory=list)
    generated_at: float = 0.0
    herdr_ok: bool = True
    inbox_ok: bool = False
    errors: list[str] = field(default_factory=list)  # sessions or records that could not be read
    docked: str | None = None  # cockpit: the orchestrator pane panorama is docked next to

    def counts(self) -> dict:
        ags = [a for o in self.orchestrators for a in o.agents()]
        return {
            "orchestrators": len(self.orchestrators),
            "threads": sum(len(o.threads) for o in self.orchestrators),
            "active_threads": sum(1 for o in self.orchestrators for t in o.threads if not t.done),
            "agents": len(ags),
            "running": sum(1 for a in ags if a.status in (RUN, REV)),
            "questions": sum(len(o.inbox) for o in self.orchestrators),
            "blocked": sum(1 for o in self.orchestrators if o.herdr_status == "blocked"),
            "failed": sum(1 for a in ags if a.status == FAIL),
        }

    def to_json(self) -> dict:
        return {"generated_at": self.generated_at, "herdr_ok": self.herdr_ok, "inbox_ok": self.inbox_ok,
                "errors": self.errors,
                "counts": self.counts(), "orchestrators": [asdict(o) for o in self.orchestrators]}
