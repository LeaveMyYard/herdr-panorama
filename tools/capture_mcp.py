"""Record herdr-panorama's MCP server (the Python one) as goldens for the Rust one.

    uv run python tools/capture_mcp.py tests/fixtures/mcp

Each session starts the server (`panorama mcp`) with every HERDR_* variable unset, its events
dir in a temp dir and a fixed CLAUDE_CODE_SESSION_ID, sends the session's requests one line
at a time, and records every line the server writes: `{"dir": ..., "msg": ...}`. A request
with an id waits for its reply; `subscriptions/listen` waits for the acknowledgement.

- `claude-code.jsonl`: Claude Code 2.1.302's own requests (`claude-code-2.1.302.requests.jsonl`,
  cut from herdr-agent-kit's fixtures/claude/mcp recording of herdr-inbox, protocol 2026-07-28).
- `legacy.jsonl`: the 2025-06-18 `initialize` handshake, an unknown method, an unknown tool.
- `report-thread.jsonl`: report_thread calls (recorded, refused, updated), then the events
  file they wrote (`"dir": "events"`, its `ts` and phase times replaced by "<ts>").
- `no-session.jsonl`: report_thread without a session id.
"""
import json
import os
import pathlib
import queue
import subprocess
import sys
import tempfile
import threading

ROOT = pathlib.Path(__file__).resolve().parent.parent
SID = "0e4a6b1c-9d2f-4a3b-8c7d-1e2f3a4b5c6d"
INIT = [{"jsonrpc": "2.0", "id": 0, "method": "initialize",
         "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "capture", "version": "0"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized"}]


def call(i, args):
    return {"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": "report_thread", "arguments": args}}


SESSIONS = {
    "legacy": INIT + [
        {"jsonrpc": "2.0", "id": 1, "method": "tools/list"},
        {"jsonrpc": "2.0", "id": 2, "method": "no/such/method"},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "no_such_tool", "arguments": {}}},
        {"jsonrpc": "2.0", "id": 4, "method": "ping"},
    ],
    "report-thread": INIT + [
        call(1, {"thread_id": "pr-595", "title": "PR #595 ledger", "phase": "review r1",
                 "phases_done": ["author"], "next": ["merge"], "agents": ["review*", "a1b2c3"]}),
        call(2, {"thread_id": "pr-595", "phase": "merge", "phases_done": ["author", "review r1"]}),
        call(3, {"thread_id": "pr-595", "status": "done"}),
        call(4, {"thread_id": "x", "status": "bogus"}),
        call(5, {"thread_id": "x", "next": [f"p{i}" for i in range(31)]}),
        call(6, {"thread_id": "x", "agents": ["ab*"]}),
        call(7, {"title": "no id"}),
        call(8, {"thread_id": "t\u202e\x07 two", "title": "x" * 250}),
        call(9, {"thread_id": "t\u00e9", "title": "\u00dcn\u00efcode \u2713", "phase": "\u00e9"}),
    ],
    "no-session": INIT + [call(1, {"thread_id": "t"})],
}


def run(requests, env, out):
    p = subprocess.Popen([str(pathlib.Path(sys.executable).parent / "panorama"), "mcp"], stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, text=True, env=env, cwd=ROOT)
    lines: queue.Queue = queue.Queue()
    threading.Thread(target=lambda: [lines.put(json.loads(x)) for x in p.stdout], daemon=True).start()
    rec = []
    for r in requests:
        rec.append({"dir": "client->server", "msg": r})
        p.stdin.write(json.dumps(r, ensure_ascii=False) + "\n")
        p.stdin.flush()
        if "id" not in r:
            continue
        listen = r["method"] == "subscriptions/listen"
        while True:
            m = lines.get(timeout=30)
            rec.append({"dir": "server->client", "msg": m})
            if listen and m.get("method") == "notifications/subscriptions/acknowledged":
                break
            if m.get("id") == r["id"]:
                break
    p.stdin.close()
    p.wait(timeout=30)
    while not lines.empty():
        rec.append({"dir": "server->client", "msg": lines.get()})
    with open(out, "w") as fh:
        for x in rec:
            fh.write(json.dumps(x, ensure_ascii=False) + "\n")
    return rec


def main(out_dir: str) -> None:
    out = pathlib.Path(out_dir)
    base = {k: v for k, v in os.environ.items() if not k.startswith(("HERDR_", "CLAUDE_", "PANORAMA_"))}
    claude = [json.loads(x) for x in (out / "claude-code-2.1.302.requests.jsonl").read_text().splitlines()]
    for name, reqs in [("claude-code", claude), *SESSIONS.items()]:
        with tempfile.TemporaryDirectory() as tmp:
            env = {**base, "PANORAMA_EVENTS_DIR": tmp, "XDG_STATE_HOME": tmp}
            if name != "no-session":
                env["CLAUDE_CODE_SESSION_ID"] = SID
            path = out / f"{name}.jsonl"
            run(reqs, env, path)
            if name == "report-thread":
                with open(path, "a") as fh:
                    for line in (pathlib.Path(tmp) / f"{SID}.jsonl").read_text().splitlines():
                        rec = json.loads(line)
                        rec["ts"] = "<ts>"
                        for ph in rec.get("phases", []):
                            for k in ("start", "end"):
                                if k in ph:
                                    ph[k] = "<ts>"
                        fh.write(json.dumps({"dir": "events", "msg": rec}, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main(sys.argv[1])
