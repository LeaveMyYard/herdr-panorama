"""A demo Swarm built directly from model dataclasses (neutral names).

Mirrors the design's sample dataset in shape: four orchestrators with
pr-review, workflow, coordinator-style and explicit threads, one question, one
blocked session and one failed agent. Used by the UI tests and the screenshots,
so they do not depend on the (provisional) transcript data layer.
"""

from __future__ import annotations

from datetime import datetime

from herdr_panorama.model import (BLK, CXL, DONE, FAIL, Q, REV, RUN, WAIT, Agent, Orchestrator, Phase, Swarm,
                                  Thread)

NOW = datetime(2026, 10, 14, 10, 4).timestamp()  # local 10:04, like the design
M = 60.0


def at(hhmm: str) -> float:
    h, m = (int(x) for x in hhmm.split(":"))
    return datetime(2026, 10, 14, h, m).timestamp()


_seq = [0]


def ag(aid, role, model, st, start, tools, phs, act, **kw) -> Agent:
    _seq[0] += 1
    a = Agent(id=aid, description=kw.pop("desc", f"{role} {aid}"), role=role, model=model, status=st,
              started=at(start), tool_calls=tools, phases=list(phs), activity=act, seq=_seq[0],
              transcript=f"/home/user/.claude/projects/-home-user-project/s/subagents/agent-{aid}.jsonl", **kw)
    if st in (DONE, CXL, FAIL):
        a.ended = a.ended or (at(start) + 10 * M)
    return a


def ph(name, state, start=None, end=None, note=None) -> Phase:
    return Phase(name, state, at(start) if start else None, at(end) if end else None, note)


def demo_swarm() -> Swarm:
    _seq[0] = 0
    billing = Orchestrator(pane_id="w1:p1", tab_id="w1:t1", tab_label="Billing API", session_id="s1",
                           herdr_status="working", flavour="pr-review",
                           transcript="/home/user/.claude/projects/-home-user-project/s1.jsonl")
    t42 = Thread(id="ref:#42", name="PR #42 invoice export", source="ref", status="active", started=at("09:02"),
                 phases=[ph("author", "done", "09:02", "09:20"), ph("review r1", "done", "09:24", "09:38", "1B 2H 3M"),
                         ph("fix r1", "done", "09:41", "09:43"), ph("review r2", "now", "09:43", None, "0B 1H 2M so far"),
                         ph("CI", "next"), ph("merge", "next")],
                 agents=[ag("a2f81c0", "reviewer", "opus", REV, "09:43", 52, [1, 3], "Bash(Diff totals.py against round 1 fixes)",
                            last_message="Round 1 Blocker (double charge on retry) is fixed. Checking the High on stale TTL next."),
                         ag("a1c3d90", "author", "opus", Q, "09:02", 131, [0, 2], "idle · waits for r2 findings",
                            last_message="Pushed fix r1 (3 commits)."),
                         ag("a3e1b22", "explorer", "haiku", DONE, "09:06", 18, [0], "mapped invoice call sites"),
                         ag("a4d0e11", "explorer", "haiku", DONE, "09:07", 11, [0], "read docs/invoices.md"),
                         ag("a5b7f02", "explorer", "haiku", DONE, "09:26", 9, [1], "pulled PR #41 context")])
    t44 = Thread(id="ref:#44", name="PR #44 health badge", source="ref", status="active", started=at("08:10"),
                 phases=[ph("author", "done", "08:10", "08:31"), ph("review r1", "done", "08:35", "08:52", "0B 1H 1M"),
                         ph("fix r1", "done", "08:58", "09:54"), ph("CI", "failed", "09:55", "10:03", "3 failing"),
                         ph("merge", "next")],
                 agents=[ag("b3a9c41", "fixer", "sonnet", FAIL, "09:55", 23, [3], "stopped after CI failure",
                            error="npm test exited 1 · 3 failing in badge.spec.ts",
                            last_message="Badge snapshot changed and 3 tests fail on a missing aria-label."),
                         ag("b1f0d33", "author", "sonnet", DONE, "08:10", 88, [0, 2], "pushed fix r1"),
                         ag("b2c4e55", "reviewer", "sonnet", DONE, "08:35", 29, [1], "posted 0B 1H 1M"),
                         ag("b4e6a77", "explorer", "haiku", DONE, "09:56", 6, [3], "fetched CI log")])
    c1 = ag("c1d2e3f", "author", "sonnet", WAIT, "09:58", 14, [0], "Bash(Grep i18n keys for retry)",
            question_ids=[2], questions=['Use "Retrying…" or "Reconnecting…" for the banner? The design doc and the i18n keys disagree.'],
            last_message="Banner component drafted; copy is blocked on wording.")
    t48 = Thread(id="ref:#48", name="PR #48 retry banner copy", source="ref", status="active", started=at("09:58"),
                 phases=[ph("author", "now", "09:58"), ph("review r1", "next"), ph("CI", "next"), ph("merge", "next")],
                 agents=[c1, ag("c2b3a4d", "explorer", "haiku", DONE, "09:59", 7, [0], "grepped i18n keys")])
    t40 = Thread(id="ref:#40", name="PR #40 currency codes", source="ref", status="done", started=at("07:10"),
                 ended=at("08:40"), phases=[ph("author", "done", "07:10", "08:00"), ph("review r1", "done", "08:02", "08:40")],
                 agents=[ag("e1a2b3c", "author", "sonnet", DONE, "07:10", 40, [0], "merged")])
    billing.threads = [t42, t44, t48, t40]
    billing.inbox, billing.inbox_texts = [2], [c1.questions[0]]

    search = Orchestrator(pane_id="w1:p2", tab_id="w1:t2", tab_label="Search refactor", session_id="s2",
                          herdr_status="working", flavour="workflow")
    wf = Thread(id="wf:wf_search", name="search-index-rebuild", source="workflow", status="active", started=at("09:20"),
                phases=[ph("Plan", "done", "09:20", "09:41"), ph("Build", "now", "09:42", None, "tokenizer, ranker"),
                        ph("Verify", "next"), ph("Merge", "next")],
                agents=[ag("j1a2b3c", "author", "sonnet", RUN, "09:42", 97, [1], "Read(src/search/tokenize.rs)",
                           last_message="Splitting on Unicode word boundaries; wiring the ranker next."),
                        ag("j4d5e6f", "author", "opus", RUN, "09:44", 41, [1], "Bash(cargo test -p ranker)"),
                        ag("j2b3c4d", "planner", "haiku", DONE, "09:20", 14, [0], "plan written"),
                        ag("j3c4d5e", "explorer", "haiku", CXL, "09:21", 3, [0], "cancelled · duplicate")])
    search.threads = [wf]

    docs = Orchestrator(pane_id="w1:p3", tab_id="w1:t3", tab_label="Docs site", session_id="s3",
                        herdr_status="blocked", flavour="docs",
                        pending=["Publish the v2 docs now or after the release?"])
    main = Agent(id="main:w1:p3", description="main session", role="orchestrator", status=BLK, started=at("09:59"),
                 activity="waiting for you in the pane", pending=list(docs.pending), phases=[0], seq=99)
    docs.threads = [
        Thread(id="main:w1:p3", name="Docs site", source="session", status="active", started=at("09:59"),
               phases=[ph("waiting on you", "now", "09:59")], agents=[main]),
        Thread(id="x:docs-v2", name="Docs v2 release", source="explicit", status="active", started=at("09:30"),
               pct=60, worktree="wt/docs-v2", flavour="docs",
               phases=[ph("outline", "done", "09:30", "09:31"), ph("write", "now", "09:31"), ph("review", "next"),
                       ph("publish", "next")],
               agents=[ag("d1e2f3a", "writer", "sonnet", RUN, "09:31", 33, [1], "Bash(Build the docs site)")]),
    ]
    notes = Orchestrator(pane_id="w1:p4", tab_id="w1:t4", tab_label="Release notes", session_id="s4",
                         herdr_status="idle", flavour="agents")
    notes.threads = [Thread(id="turn:0", name="Collect merged PR titles", source="turn", status="done",
                            started=at("08:05"), ended=at("08:15"), phases=[ph("explore", "done", "08:05", "08:15")],
                            agents=[ag("r1a2b3c", "explorer", "haiku", DONE, "08:05", 12, [0], "listed merged PRs")])]
    return Swarm(orchestrators=[billing, search, docs, notes], generated_at=NOW, herdr_ok=True, inbox_ok=True)
