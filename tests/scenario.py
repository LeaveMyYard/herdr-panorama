"""Build Claude Code project dirs, herdr snapshots and inbox databases for tests.

Every record is a deep copy of a template captured from a real file
(tests/fixtures/templates.json, herdr_snapshot.json; see capture.py) with ids,
times and text filled in. Nothing about the wire format is invented here.
"""

from __future__ import annotations

import copy
import json
import sqlite3
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

FIX = Path(__file__).parent / "fixtures"
T = json.loads((FIX / "templates.json").read_text())
SNAP = json.loads((FIX / "herdr_snapshot.json").read_text())

BASE = datetime(2026, 10, 14, 9, 0, tzinfo=timezone.utc).timestamp()
CWD = "/home/user/project"
SLUG = "-home-user-project"


def iso(t: float) -> str:
    return datetime.fromtimestamp(t, timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.") + "%03dZ" % int((t % 1) * 1000)


def tpl(kind: str) -> dict:
    return copy.deepcopy(T[kind])


def stamp(rec: dict, t: float, session: str, agent_id: str | None = None) -> dict:
    rec["timestamp"] = iso(t)
    rec["uuid"] = str(uuid.uuid4())
    for key in ("sessionId", "session_id"):
        if key in rec:
            rec[key] = session
    if "cwd" in rec:
        rec["cwd"] = CWD
    if agent_id is not None:
        rec["agentId"] = agent_id
    return rec


def tool_block(rec: dict, name: str, tid: str, inp: dict) -> dict:
    block = next(b for b in rec["message"]["content"] if b.get("type") == "tool_use")
    block.update(name=name, id=tid, input=inp)
    rec["message"]["content"] = [block]
    return rec


def result_block(rec: dict, tid: str, text: str, is_error: bool | None = None) -> dict:
    block = next(b for b in rec["message"]["content"] if b.get("type") == "tool_result")
    block["tool_use_id"] = tid
    block["content"] = [{"type": "text", "text": text}]
    if is_error is not None:
        block["is_error"] = is_error
    rec["message"]["content"] = [block]
    return rec


def notification(agent_id: str, tool_use_id: str, status: str, tool_uses: int = 0) -> str:
    return ("<task-notification>\n"
            f"<task-id>{agent_id}</task-id>\n<tool-use-id>{tool_use_id}</tool-use-id>\n"
            f"<output-file>/tmp/tasks/{agent_id}.output</output-file>\n<status>{status}</status>\n"
            f"<summary>Agent finished</summary>\n<result>report</result>\n"
            f"<usage><subagent_tokens>1000</subagent_tokens><tool_uses>{tool_uses}</tool_uses>"
            f"<duration_ms>1000</duration_ms></usage>\n</task-notification>")


class Transcript:
    """Writes one transcript (main session or an agent's own file)."""

    def __init__(self, path: Path, session: str, agent_id: str | None = None):
        self.path, self.session, self.agent_id = path, session, agent_id
        path.parent.mkdir(parents=True, exist_ok=True)
        self.lines: list[str] = []
        self._n = 0

    def tid(self) -> str:
        self._n += 1
        return f"toolu_{abs(hash((str(self.path), self._n))) % 10**22:022d}"

    def add(self, rec: dict, t: float) -> None:
        self.lines.append(json.dumps(stamp(rec, t, self.session, self.agent_id), separators=(",", ":")))

    def raw(self, line: str) -> None:
        self.lines.append(line)

    def prompt(self, t: float, text: str, legacy: bool = False) -> None:
        """An operator prompt (``origin.kind == "human"``; ``legacy``: an older record without origin)."""
        rec = tpl("sub_first_user" if self.agent_id else ("user_prompt" if legacy else "user_prompt_human"))
        rec["message"]["content"] = text
        if legacy:
            rec.pop("origin", None)
        self.add(rec, t)

    def title(self, kind: str, value: str) -> None:
        """custom-title / agent-name / ai-title records (undated metadata lines)."""
        key = {"custom_title": "customTitle", "agent_name": "agentName", "ai_title": "aiTitle"}[kind]
        rec = tpl(kind)
        rec[key] = value
        if "sessionId" in rec:
            rec["sessionId"] = self.session
        self.lines.append(json.dumps(rec, separators=(",", ":")))

    def text(self, t: float, text: str) -> None:
        rec = tpl("sub_assistant_text" if self.agent_id else "assistant_text")
        block = next(b for b in rec["message"]["content"] if b.get("type") == "text")
        block["text"] = text
        rec["message"]["content"] = [block]
        self.add(rec, t)

    def bash(self, t: float, description: str, output: str = "ok", pending: bool = False) -> None:
        """A Bash call and its result; ``pending``: no result yet (in flight, or abandoned)."""
        tid = self.tid()
        rec = tpl("sub_assistant_bash" if self.agent_id else "assistant_bash")
        self.add(tool_block(rec, "Bash", tid, {"command": "true", "description": description}), t)
        if pending:
            return
        res = tpl("sub_result_bash" if self.agent_id else "result_bash")
        res["toolUseResult"].update(stdout=output, stderr="")
        self.add(result_block(res, tid, output, False), t + 1)

    def handback(self, t: float, report: str = "done") -> None:
        tid = self.tid()
        self.add(tool_block(tpl("sub_assistant_handback"), "SubagentHandback", tid, {"message": report}), t)

    def launch(self, t: float, agent_id: str, description: str, model: str = "opus", background: bool = True,
               subagent_type: str = "general-purpose") -> str:
        tid = self.tid()
        inp = {"description": description, "subagent_type": subagent_type, "model": model, "prompt": "brief"}
        if background:
            inp["run_in_background"] = True
        self.add(tool_block(tpl("sub_assistant_agent" if self.agent_id else "assistant_agent"), "Agent", tid, inp), t)
        if background:
            res = tpl("sub_result_agent_async" if self.agent_id else "result_agent_async")
            res["toolUseResult"].update(agentId=agent_id, description=description, status="async_launched")
            self.add(result_block(res, tid, f"Async agent launched successfully.\nagentId: {agent_id}"), t + 1)
        return tid

    def foreground_done(self, t: float, tid: str, agent_id: str, tool_count: int, error: bool = False) -> None:
        res = tpl("result_agent_foreground")
        res["toolUseResult"].update(agentId=agent_id, status="completed", totalToolUseCount=tool_count)
        self.add(result_block(res, tid, "report", error), t)

    def notify(self, t: float, agent_id: str, tool_use_id: str, status: str = "completed", tool_uses: int = 0,
               carriers: tuple[str, ...] = ("queue", "attachment")) -> None:
        """A <task-notification>, written the way real files carry it: in the main transcript as a
        queue-operation enqueue plus a queued_command attachment; in an agent's file (the
        spawner of a nested agent) as a queued_command attachment only; ``"user"`` as a plain
        user string (older sessions)."""
        text = notification(agent_id, tool_use_id, status, tool_uses)
        if self.agent_id:
            carriers = tuple(c for c in carriers if c != "queue")
        if "queue" in carriers:
            q = tpl("queue_enqueue_notification")
            q["content"] = text
            self.add(q, t)
        if "attachment" in carriers:
            att = tpl("sub_attachment_queued_command" if self.agent_id else "attachment_queued_command")
            att["attachment"]["prompt"] = text
            self.add(att, t + 0.01)
        if "user" in carriers:
            rec = tpl("user_prompt")
            rec.pop("origin", None)
            rec["message"]["content"] = text
            self.add(rec, t + 0.02)

    def resume(self, t: float, agent_id: str, message: str = "continue") -> None:
        """SendMessage to a finished agent: the result names ``resumedAgentId``."""
        tid = self.tid()
        self.add(tool_block(tpl("assistant_sendmessage"), "SendMessage", tid, {"to": agent_id, "message": message}), t)
        res = tpl("result_sendmessage_resume")
        res["toolUseResult"].update(success=True, message=f"Resuming agent {agent_id[:7]}", resumedAgentId=agent_id,
                                    pin={"id": agent_id, "name": agent_id, "ref": "000000"})
        self.add(result_block(res, tid, json.dumps(res["toolUseResult"])), t + 1)

    def send_message(self, t: float, to: str, message: str = "continue") -> None:
        tid = self.tid()
        self.add(tool_block(tpl("assistant_sendmessage"), "SendMessage", tid, {"to": to, "message": message}), t)
        res = tpl("result_sendmessage")
        msg = f"Message queued for delivery to {to} at its next tool round."
        res["toolUseResult"].update(success=True, message=msg)
        self.add(result_block(res, tid, msg), t + 1)

    def workflow(self, t: float, run_id: str, task_id: str, name: str) -> str:
        tid = self.tid()
        self.add(tool_block(tpl("assistant_workflow"), "Workflow", tid, {"script": "export const meta = {}"}), t)
        res = tpl("result_workflow")
        res["toolUseResult"].update(status="async_launched", taskId=task_id, runId=run_id, workflowName=name,
                                    taskType="local_workflow")
        self.add(result_block(res, tid, f"Workflow launched in background. Task ID: {task_id}"), t + 1)
        return tid

    def ask_user_question(self, t: float, questions: list[str]) -> None:
        tid = self.tid()
        inp = {"questions": [{"question": q, "header": "Choice", "multiSelect": False,
                              "options": [{"label": "yes", "description": ""}, {"label": "no", "description": ""}]}
                             for q in questions]}
        self.add(tool_block(tpl("assistant_askuserquestion"), "AskUserQuestion", tid, inp), t)

    def ask_operator(self, t: float, qid: int, question: str) -> None:
        tid = self.tid()
        name = "mcp__plugin_herdr-inbox_herdr-inbox__ask_operator"
        self.add(tool_block(tpl("assistant_ask_operator"), name, tid,
                            {"questions": [{"question": question}], "asked_by": "agent"}), t)
        text = json.dumps({"group": qid, "questions": [{"id": qid, "status": "open",
                                                        "message": f"Recorded as Q{qid}. Continue."}]})
        self.add(result_block(tpl("result_ask_operator"), tid, text), t + 1)

    def write(self, partial: str | None = None) -> None:
        body = "\n".join(self.lines) + "\n"
        if partial:
            body += partial  # a last line still being written
        self.path.write_text(body)


class Project:
    def __init__(self, root: Path):
        self.root = root
        self.dir = root / SLUG
        self.dir.mkdir(parents=True, exist_ok=True)

    def main(self, session: str) -> Transcript:
        return Transcript(self.dir / f"{session}.jsonl", session)

    def agent(self, session: str, agent_id: str, meta: dict, run_id: str | None = None,
              kind: str = "meta_background") -> Transcript:
        sub = self.dir / session / "subagents"
        if run_id:
            sub = sub / "workflows" / run_id
        sub.mkdir(parents=True, exist_ok=True)
        m = tpl(kind)
        for key in list(m):
            if key in meta:
                m[key] = meta[key]
        m.update({k: v for k, v in meta.items() if k in ("parentAgentId", "workflowPhase", "toolUseId", "spawnDepth")})
        if "parentAgentId" in m and "parentAgentId" not in meta:
            del m["parentAgentId"]
        if "toolUseId" in m and "toolUseId" not in meta:
            del m["toolUseId"]  # the template's placeholder; real metas name the spawning call
        (sub / f"agent-{agent_id}.meta.json").write_text(json.dumps(m))
        return Transcript(sub / f"agent-{agent_id}.jsonl", session, agent_id)

    def journal(self, session: str, run_id: str, events: list[dict]) -> None:
        d = self.dir / session / "subagents" / "workflows" / run_id
        d.mkdir(parents=True, exist_ok=True)
        lines = []
        for ev in events:
            rec = tpl(f"journal_{ev['type']}")
            for key in list(rec):
                if key in ev:
                    rec[key] = ev[key]
            if "result" in rec:
                rec["result"] = ev.get("result", {})
            lines.append(json.dumps(rec))
        (d / "journal.jsonl").write_text("\n".join(lines) + "\n")

    def workflow_summary(self, session: str, run_id: str, name: str, status: str, phases: list[str]) -> None:
        d = self.dir / session / "workflows"
        d.mkdir(parents=True, exist_ok=True)
        rec = tpl("workflow_summary")
        rec.update(runId=run_id, workflowName=name, status=status, phases=[{"title": p} for p in phases],
                   workflowProgress=[], logs=[])
        (d / f"{run_id}.json").write_text(json.dumps(rec))


def snapshot(panes: list[dict]) -> dict:
    """herdr api snapshot from the captured template: panes = [{pane, tab, label, status, session, cwd}]."""
    snap = copy.deepcopy(SNAP["result"]["snapshot"])
    agent_t, tab_t, pane_t = snap["agents"][0], snap["tabs"][0], snap["panes"][0]
    snap["agents"], snap["tabs"], snap["panes"] = [], [], []
    for p in panes:
        a = copy.deepcopy(agent_t)
        a.update(pane_id=p["pane"], tab_id=p["tab"], workspace_id="w1", agent=p.get("agent", "claude"),
                 agent_status=p.get("status", "working"), cwd=p.get("cwd", CWD), focused=p.get("focused", False),
                 terminal_title_stripped=p.get("title", ""))
        if p.get("session"):
            a["agent_session"].update(agent="claude", kind="id", source="herdr:claude", value=p["session"])
        else:
            a.pop("agent_session", None)
        snap["agents"].append(a)
        pn = copy.deepcopy(pane_t)
        pn.update(pane_id=p["pane"], tab_id=p["tab"], workspace_id="w1")
        snap["panes"].append(pn)
        if not any(t["tab_id"] == p["tab"] for t in snap["tabs"]):
            t = copy.deepcopy(tab_t)
            t.update(tab_id=p["tab"], label=p.get("label", ""), workspace_id="w1")
            snap["tabs"].append(t)
    return snap


INBOX_SCHEMA = """
CREATE TABLE IF NOT EXISTS questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT, created_at TEXT NOT NULL, pane_id TEXT, tab_id TEXT,
    tab_label TEXT, session_id TEXT, cwd TEXT, asked_by TEXT, question TEXT NOT NULL, options TEXT,
    context TEXT, norm TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'open', answer TEXT, answered_at TEXT,
    delivered_at TEXT, delivery_error TEXT, acknowledged_at TEXT, repeat_count INTEGER NOT NULL DEFAULT 0,
    delivery_attempts INTEGER NOT NULL DEFAULT 0
);
"""  # herdr-inbox's 0.1 questions table (src/herdr_inbox/store.py)


def inbox_db(path: Path, rows: list[dict]) -> Path:
    con = sqlite3.connect(path)
    con.executescript(INBOX_SCHEMA)
    for r in rows:
        con.execute("INSERT INTO questions (id, created_at, pane_id, session_id, asked_by, question, norm, status)"
                    " VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    (r["id"], "2026-10-14T09:00:00Z", r.get("pane"), r.get("session"), r.get("asked_by"),
                     r["question"], r["question"].lower(), r.get("status", "open")))
    con.commit()
    con.close()
    return path


# -- the demo swarm (neutral names; used by tests and the screenshots) -------------------

S1, S2, S3, S4 = ("11111111-1111-4111-8111-111111111111", "22222222-2222-4222-8222-222222222222",
                  "33333333-3333-4333-8333-333333333333", "44444444-4444-4444-8444-444444444444")
NOW = BASE + 70 * 60
M = 60


def build_demo(tmp: Path) -> dict:
    """A small swarm: PR review cycle, a workflow, a blocked session, an explicit thread."""
    proj = Project(tmp / "projects")
    # -- Billing API: PR review cycles (ref threads), a failed fixer, a question ----------
    m = proj.main(S1)
    m.prompt(BASE, "Take PR #42 and PR #44 through review")
    t_a1 = m.launch(BASE + 2 * M, "a1author0000000001", "Author PR #42 invoice export")
    a1 = proj.agent(S1, "a1author0000000001", {"description": "Author PR #42 invoice export", "model": "opus",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    a1.prompt(BASE + 2 * M, "brief")
    a1.bash(BASE + 4 * M, "Read invoice exporter")
    t_a3 = a1.launch(BASE + 5 * M, "a3explore000000001", "Explore invoice call sites", model="haiku",
                     subagent_type="Explore")
    a3 = proj.agent(S1, "a3explore000000001", {"description": "Explore invoice call sites", "model": "haiku",
                                               "agentType": "Explore", "spawnDepth": 2, "toolUseId": t_a3},
                    kind="meta_nested")
    a3.prompt(BASE + 5 * M, "brief")
    a3.bash(BASE + 6 * M, "Find callers of export_invoice")
    a3.text(BASE + 9 * M, "Three call sites, all in billing/export.py.")
    a1.notify(BASE + 9 * M, "a3explore000000001", t_a3, "completed", 9)
    a1.bash(BASE + 12 * M, "Run invoice tests")
    a1.text(BASE + 19 * M, "Pushed the invoice export; tests pass.")
    m.notify(BASE + 20 * M, "a1author0000000001", t_a1, "completed", 31)
    t_a2 = m.launch(BASE + 22 * M, "a2review0000000001", "Review PR #42 round 1")
    a2 = proj.agent(S1, "a2review0000000001", {"description": "Review PR #42 round 1", "model": "opus",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    a2.prompt(BASE + 22 * M, "brief")
    a2.bash(BASE + 25 * M, "Diff PR 42 against base")
    a2.handback(BASE + 38 * M, "1 High, 2 Medium")
    m.notify(BASE + 38 * M, "a2review0000000001", t_a2, "completed", 14)
    m.resume(BASE + 41 * M, "a1author0000000001", "fix the High")
    a1.bash(BASE + 43 * M, "Fix rounding in totals")
    a1.text(BASE + 46 * M, "Fixed the rounding High; pushing.")
    m.notify(BASE + 47 * M, "a1author0000000001", t_a1, "completed", 44)
    t_a4 = m.launch(BASE + 50 * M, "a4review0000000002", "Review PR #42 round 2")
    a4 = proj.agent(S1, "a4review0000000002", {"description": "Review PR #42 round 2", "model": "opus",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    a4.prompt(BASE + 50 * M, "brief")
    a4.bash(BASE + 66 * M, "Diff totals.py against round 1 fixes")
    a4.text(BASE + 67 * M, "Round 1 High is fixed. Checking the Medium on currency codes next.")
    t_b1 = m.launch(BASE + 3 * M, "b1author0000000001", "Author PR #44 health badge", model="sonnet")
    b1 = proj.agent(S1, "b1author0000000001", {"description": "Author PR #44 health badge", "model": "sonnet",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    b1.prompt(BASE + 3 * M, "brief")
    b1.handback(BASE + 30 * M)
    m.notify(BASE + 30 * M, "b1author0000000001", t_b1, "completed", 22)
    t_b3 = m.launch(BASE + 55 * M, "b3fixer00000000001", "Fix CI for PR #44", model="sonnet")
    b3 = proj.agent(S1, "b3fixer00000000001", {"description": "Fix CI for PR #44", "model": "sonnet",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    b3.prompt(BASE + 55 * M, "brief")
    b3.bash(BASE + 60 * M, "Run badge tests")
    b3.text(BASE + 68 * M, "npm test exited 1: 3 failing in badge.spec.ts. Stopping per the CI rule.")
    m.notify(BASE + 69 * M, "b3fixer00000000001", t_b3, "failed", 23)
    m.prompt(BASE + 58 * M, "Draft the retry banner copy")
    m.launch(BASE + 59 * M, "c1banner0000000001", "Draft retry banner copy", model="sonnet")
    c1 = proj.agent(S1, "c1banner0000000001", {"description": "Draft retry banner copy", "model": "sonnet",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    c1.prompt(BASE + 59 * M, "brief")
    c1.bash(BASE + 62 * M, "Grep i18n keys for retry")
    c1.ask_operator(BASE + 64 * M, 2, "Use \"Retrying…\" or \"Reconnecting…\" for the banner?")
    m.text(BASE + 60 * M, "Started the banner copy agent.")
    for t in (m, a1, a2, a3, a4, b1, b3, c1):
        t.write()
    # -- Search refactor: a native workflow run --------------------------------------------
    m2 = proj.main(S2)
    m2.prompt(BASE + 10 * M, "Rebuild the search index with a workflow")
    m2.workflow(BASE + 11 * M, "wf_search-01", "wtask0001", "search-index-rebuild")
    jr = [{"type": "launched"},
          {"type": "started", "agentId": "w1plan000000000001", "label": "plan:index", "phase": "Plan"},
          {"type": "result", "agentId": "w1plan000000000001", "result": {"ok": True}},
          {"type": "started", "agentId": "w2build00000000001", "label": "build:tokenizer", "phase": "Build"},
          {"type": "started", "agentId": "w3build00000000002", "label": "build:ranker", "phase": "Build"}]
    proj.journal(S2, "wf_search-01", jr)
    for aid, label, phase, start, last in (("w1plan000000000001", "plan:index", "Plan", 11, 20),
                                           ("w2build00000000001", "build:tokenizer", "Build", 21, 69),
                                           ("w3build00000000002", "build:ranker", "Build", 22, 68)):
        w = proj.agent(S2, aid, {"description": label, "model": "opus", "workflowPhase": phase},
                       run_id="wf_search-01", kind="meta_workflow")
        w.prompt(BASE + start * M, "brief")
        w.bash(BASE + last * M, f"Work on {label.split(':')[1]}")
        w.write()
    # phases are declared in the run JSON only when the run ends; while live, the journal order
    m2.write()
    # -- Docs site: blocked on AskUserQuestion; explicit channel thread ------------------------
    m3 = proj.main(S3)
    m3.prompt(BASE + 30 * M, "Prepare the v2 docs")
    m3.launch(BASE + 31 * M, "d1guide00000000001", "Write v2 migration guide", model="sonnet")
    d1 = proj.agent(S3, "d1guide00000000001", {"description": "Write v2 migration guide", "model": "sonnet",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    d1.prompt(BASE + 31 * M, "brief")
    d1.bash(BASE + 65 * M, "Build the docs site")
    d1.write()
    m3.ask_user_question(BASE + 66 * M, ["Publish the v2 docs now or after the release?"])
    m3.write()
    events_dir = tmp / "events"
    events_dir.mkdir(exist_ok=True)
    (events_dir / f"{S3}.jsonl").write_text(json.dumps({
        "v": 1, "ts": iso(BASE + 31 * M), "thread": "docs-v2", "name": "Docs v2 release", "flavour": "docs",
        "status": "active", "pct": 60, "phases": [
            {"name": "outline", "state": "done", "start": iso(BASE + 30 * M), "end": iso(BASE + 31 * M)},
            {"name": "write", "state": "now", "start": iso(BASE + 31 * M)},
            {"name": "review", "state": "next"}, {"name": "publish", "state": "next"}],
        "agents": [{"match": "Write v2*", "role": "writer", "phase": 1}]}) + "\n")
    # -- Release notes: idle, one finished agent ----------------------------------------------
    m4 = proj.main(S4)
    m4.prompt(BASE + 5 * M, "Collect release notes")
    t_r = m4.launch(BASE + 6 * M, "r1notes00000000001", "Collect merged PR titles", model="haiku")
    r1 = proj.agent(S4, "r1notes00000000001", {"description": "Collect merged PR titles", "model": "haiku",
                                               "agentType": "general-purpose", "spawnDepth": 1})
    r1.prompt(BASE + 6 * M, "brief")
    r1.handback(BASE + 15 * M)
    r1.write()
    m4.notify(BASE + 15 * M, "r1notes00000000001", t_r, "completed", 12)
    m4.write()
    snap = snapshot([
        {"pane": "w1:p1", "tab": "w1:t1", "label": "Billing API", "status": "working", "session": S1},
        {"pane": "w1:p2", "tab": "w1:t2", "label": "Search refactor", "status": "working", "session": S2},
        {"pane": "w1:p3", "tab": "w1:t3", "label": "Docs site", "status": "blocked", "session": S3},
        {"pane": "w1:p4", "tab": "w1:t4", "label": "Release notes", "status": "idle", "session": S4},
        {"pane": "w1:p5", "tab": "w1:t5", "label": "shell", "status": "unknown", "agent": "shell"},
    ])
    db = inbox_db(tmp / "inbox.db", [
        {"id": 1, "session": S1, "pane": "w1:p1", "question": "Merge PR #41 with a merge commit or squash?"},
        {"id": 2, "session": S1, "pane": "w1:p1", "question": "Use \"Retrying…\" or \"Reconnecting…\" for the banner?"},
        {"id": 3, "session": S4, "pane": "w1:p4", "question": "Old question", "status": "answered"},
    ])
    return {"projects": proj.root, "snapshot": snap, "inbox": db, "events": events_dir, "now": NOW, "project": proj}


def set_mtimes(root: Path, t: float) -> None:
    """Make every file look last written at ``t`` (the collector compares mtimes with its clock)."""
    import os
    for p in root.rglob("*"):
        if p.is_file():
            os.utime(p, (t, t))


def touch_running(root: Path, names: list[str], t: float) -> None:
    import os
    for p in root.rglob("*.jsonl"):
        if any(n in p.name for n in names):
            os.utime(p, (t, t))


__all__ = ["build_demo", "Project", "Transcript", "snapshot", "inbox_db", "BASE", "NOW", "S1", "S2", "S3", "S4",
           "set_mtimes", "touch_running", "time"]
