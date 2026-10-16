"""Grouping agents into threads and deriving phases.

Sources, strongest first; an agent belongs to the first that claims it:
explicit channel -> native Workflow run -> PR/issue reference -> operator turn.
Nested agents join their root agent's thread. See docs/DESIGN.md.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from .model import CXL, DONE, FAIL, RUN, Agent, Phase, Thread, live, urgency

ROLE_WORDS = [
    ("reviewer", ("review", "verif")),
    ("fixer", ("fix", "cifix")),
    ("merger", ("merge",)),
    ("author", ("author", "implement", "build", "write", "draft", "rebase")),
    ("explorer", ("explor", "research", "investigat", "find", "map", "search", "survey", "read", "look")),
    ("planner", ("plan",)),
]
PHASE_WORD = {"reviewer": "review", "fixer": "fix", "author": "author", "explorer": "explore",
              "merger": "merge", "planner": "plan"}
NUMBERED = {"review", "fix"}

_REF = re.compile(r"(?:\b([A-Za-z][\w.-]*)\s+)?(?:\bPR\s*#?\s*|pull/|#)(\d{1,6})\b", re.IGNORECASE)
_STOP = {"review", "re-review", "rereview", "merge", "fix", "fixes", "rebase", "author", "for", "of", "on",
         "in", "the", "and", "to", "pr", "issue", "check", "ci", "a", "an", "re", "reviewer", "close",
         "author-rebase", "with", "from", "after", "before", "about", "is", "at", "by", "into", "draft"}


def role_of(*texts: str | None, fallback: str | None = None) -> str:
    """The first role keyword found in the texts (description, workflow label), in order."""
    for text in texts:
        low = (text or "").lower()
        best = None
        for role, words in ROLE_WORDS:
            for w in words:
                m = re.search(r"(?<![a-z])" + w, low)
                if m and (best is None or m.start() < best[0]):
                    best = (m.start(), role)
        if best:
            return best[1]
    return fallback or "agent"


def ref_key(description: str) -> tuple[str, str] | None:
    """(key, display name) for the first PR/issue reference in a description."""
    m = _REF.search(description or "")
    if not m:
        return None
    repo, num = m.group(1), m.group(2)
    if repo and (repo.lower() in _STOP or repo.isdigit()):
        repo = None
    if repo:
        return f"{repo.lower()}#{num}", f"{repo} #{num}"
    return f"#{num}", f"#{num}"


@dataclass
class Span:
    agent: Agent
    start: float | None
    end: float | None
    outcome: str  # run | done | fail | cxl


def heuristic_phases(spans: list[Span]) -> list[Phase]:
    """One phase per run of consecutive same-role spans; reviews and fixes numbered by round."""
    spans = sorted(spans, key=lambda s: (s.start or 0.0))
    phases: list[Phase] = []
    roles: list[str] = []
    counts: dict[str, int] = {}
    for sp in spans:
        word = PHASE_WORD.get(sp.agent.role, sp.agent.role)
        if not roles or roles[-1] != word:
            counts[word] = counts.get(word, 0) + 1
            n = counts[word]
            name = f"{word} r{n}" if word in NUMBERED else (word if n == 1 else f"{word} {n}")
            phases.append(Phase(name, "now", sp.start, sp.end))
            roles.append(word)
            spans_of = [sp]
            phases[-1]._spans = spans_of  # type: ignore[attr-defined]
        else:
            phases[-1]._spans.append(sp)  # type: ignore[attr-defined]
        idx = len(phases) - 1
        if idx not in sp.agent.phases:
            sp.agent.phases.append(idx)
    for i, ph in enumerate(phases):
        sps = ph._spans  # type: ignore[attr-defined]
        del ph._spans  # type: ignore[attr-defined]
        running = any(s.end is None for s in sps)
        ph.start = min((s.start for s in sps if s.start), default=None)
        ph.end = None if running else max((s.end for s in sps if s.end), default=None)
        last = max(sps, key=lambda s: (s.end or 0.0))
        if running:
            ph.state = "now"
        elif last.outcome == FAIL and i == len(phases) - 1:
            ph.state = "failed"
        elif all(s.outcome == CXL for s in sps):
            ph.state = "skipped"
        else:
            ph.state = "done"
    return phases


def thread_status(agents: list[Agent]) -> str:
    if any(live(a.status) and a.status != FAIL for a in agents):
        return "active"
    if not agents:
        return "done"
    newest = max(agents, key=lambda a: (a.ended or a.last_at or 0.0))
    if newest.status == FAIL:
        return "failed"
    if all(a.status == CXL for a in agents):
        return "cancelled"
    return "done"


def finish(thread: Thread) -> Thread:
    """Fill status (unless set), start/end, and order agents by urgency."""
    ags = thread.agents
    if thread.status == "":
        thread.status = thread_status(ags)
    starts = [a.started for a in ags if a.started]
    thread.started = thread.started or (min(starts) if starts else None)
    if thread.done or thread.status == "failed":
        ends = [a.ended or a.last_at for a in ags if (a.ended or a.last_at)]
        thread.ended = thread.ended or (max(ends) if ends else thread.started)
    ags.sort(key=lambda a: (urgency(a.status), a.started or 0.0))
    return thread


def sort_threads(threads: list[Thread]) -> list[Thread]:
    def key(t: Thread):
        top = min((urgency(a.status) for a in t.agents), default=99)
        return (t.done, t.status != "failed" and top, -(t.started or 0.0) if t.done else (t.started or 0.0))
    return sorted(threads, key=key)


def group_refs_and_turns(agents: list[Agent], turn_of: dict[str, tuple[int, str]],
                         spans_of: dict[str, list[tuple]]) -> list[Thread]:
    """Threads for depth-1 agents not claimed by stronger sources."""
    keyed: dict[str, list[Agent]] = {}
    names: dict[str, str] = {}
    rest: list[Agent] = []
    for a in agents:
        rk = ref_key(a.description)
        if rk:
            keyed.setdefault(rk[0], []).append(a)
            names.setdefault(rk[0], rk[1])
        else:
            rest.append(a)
    # a bare "#123" joins the one repo-qualified "<repo>#123" thread, if exactly one exists
    for key in [k for k in keyed if k.startswith("#")]:
        full = [k for k in keyed if not k.startswith("#") and k.endswith(key)]
        if len(full) == 1:
            keyed[full[0]].extend(keyed.pop(key))
    threads = []
    for key, ags in keyed.items():
        threads.append(_heuristic_thread(f"ref:{key}", names[key], "ref", ags, spans_of))
    turns: dict[int, list[Agent]] = {}
    for a in rest:
        turns.setdefault(turn_of.get(a.id, (-1, ""))[0], []).append(a)
    for idx, ags in turns.items():
        ags.sort(key=lambda a: a.started or 0.0)
        name = ags[0].description or ags[0].id
        if len(ags) > 1:
            name += f" +{len(ags) - 1}"
        threads.append(_heuristic_thread(f"turn:{idx}", name, "turn", ags, spans_of))
    return threads


def _heuristic_thread(tid: str, name: str, source: str, agents: list[Agent],
                      spans_of: dict[str, list[tuple]]) -> Thread:
    spans = []
    for a in agents:
        for start, end, outcome in spans_of.get(a.id) or [(a.started, a.ended if not live(a.status) else None,
                                                             a.status if not live(a.status) else RUN)]:
            spans.append(Span(a, start, end, outcome))
    return Thread(id=tid, name=name, source=source, phases=heuristic_phases(spans), agents=list(agents), status="")


def workflow_phases(order: list[str], agents: list[Agent], phase_of: dict[str, str | None],
                    run_live: bool, detail: dict[str, str] | None = None) -> list[Phase]:
    phases = []
    for i, title in enumerate(order):
        ags = [a for a in agents if phase_of.get(a.id) == title]
        for a in ags:
            if i not in a.phases:
                a.phases.append(i)
        starts = [a.started for a in ags if a.started]
        ends = [a.ended or a.last_at for a in ags if (a.ended or a.last_at)]
        if not ags:
            state = "next" if run_live else "skipped"
        elif any(live(a.status) for a in ags):
            state = "now"
        elif max(ags, key=lambda a: a.ended or 0.0).status == FAIL:
            state = "failed"
        else:
            state = "done"
        phases.append(Phase(title, state, min(starts) if starts else None,
                            None if state == "now" else (max(ends) if ends else None),
                            (detail or {}).get(title) or None))
    return phases


__all__ = ["role_of", "ref_key", "heuristic_phases", "group_refs_and_turns", "workflow_phases",
           "thread_status", "finish", "sort_threads", "DONE"]
