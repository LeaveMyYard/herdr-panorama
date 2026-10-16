"""``panorama`` command line: tui, snapshot, emit, open."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time


def build(collector=None):
    from . import herdr, inbox
    from .collect import Collector
    collector = collector or Collector()
    return collector.collect(herdr.snapshot(), inbox.open_questions())


def cmd_snapshot(args) -> int:
    t0 = time.monotonic()
    swarm = build()
    if args.counts:
        out = swarm.counts()
        out["herdr_ok"], out["inbox_ok"] = swarm.herdr_ok, swarm.inbox_ok
        out["by_source"] = {}
        out["agent_status"] = {}
        for o in swarm.orchestrators:
            for t in o.threads:
                out["by_source"][t.source] = out["by_source"].get(t.source, 0) + 1
                for a in t.agents:
                    out["agent_status"][a.status] = out["agent_status"].get(a.status, 0) + 1
        out["hidden_done_threads"] = sum(o.hidden_done for o in swarm.orchestrators)
        out["seconds"] = round(time.monotonic() - t0, 3)
        print(json.dumps(out, indent=2))
    elif args.json:
        print(json.dumps(swarm.to_json(), indent=2, ensure_ascii=False))
    else:
        from .model import GLYPH
        for o in swarm.orchestrators:
            marks = " ".join(m for m in ("!" if o.herdr_status == "blocked" else "", f"?{len(o.inbox)}" if o.inbox else "") if m)
            print(f"{o.tab_label}  [{o.herdr_status}] {o.flavour} {marks}")
            for t in o.threads:
                print(f"  {'✓' if t.done else '•'} {t.name}  ({t.source}, {t.status}, {len(t.agents)} agents)")
                for a in t.agents:
                    print(f"      {GLYPH.get(a.status, '?')} {a.role:<9} {a.id[:10]} {a.model or '':<6} {a.activity[:60]}")
    return 0


def cmd_tui(args) -> int:
    from .tui import SwarmApp
    app = SwarmApp()
    app.run()
    if app.after_exit is not None:  # e.g. open the inbox popup, once this popup has closed
        app.after_exit()
    return 0


def cmd_open(args) -> int:
    from . import herdr, home
    if not args.split and not args.popup and home.enabled():
        _, err = home.ensure(focus=True)  # focus the home tab (open it if missing)
        if err:
            print(f"panorama: home tab: {err}; opening the popup", file=sys.stderr)
        else:
            return 0
    err = herdr.open_view(split=args.split)
    if err:
        print(f"panorama: could not open the view: {err}", file=sys.stderr)
        return 1
    return 0


def cmd_emit(args) -> int:
    from . import events
    session = args.session or os.environ.get("CLAUDE_CODE_SESSION_ID")
    if not session:
        print("panorama emit: no session (pass --session or run inside Claude Code)", file=sys.stderr)
        return 2
    rec: dict = {"thread": args.thread}
    for key in ("name", "flavour", "status", "worktree"):
        if getattr(args, key):
            rec[key] = getattr(args, key)
    if args.pct is not None:
        rec["pct"] = args.pct
    if args.phase:
        phases = []
        for spec in args.phase:  # "name=state[@start[-end]][:note]"
            name, _, rest = spec.partition("=")
            state, _, note = rest.partition(":")
            state, _, times = state.partition("@")
            start, _, end = times.partition("-")
            p = {"name": name, "state": state or "next"}
            if start:
                p["start"] = start
            if end:
                p["end"] = end
            if note:
                p["note"] = note
            phases.append(p)
        rec["phases"] = phases
    if args.agent:
        agents = []
        for spec in args.agent:  # "match[=role[@phase]]"
            match, _, rest = spec.partition("=")
            role, _, phase = rest.partition("@")
            a: dict = {"match": match}
            if role:
                a["role"] = role
            if phase.isdigit():
                a["phase"] = int(phase)
            agents.append(a)
        rec["agents"] = agents
    if args.json:
        rec.update(json.loads(args.json))
    print(events.emit(session, rec))
    return 0


def cmd_hook(args) -> int:
    from . import hooks  # stdlib only: hooks must start fast
    return hooks.main(args.kind)


def cmd_mcp(args) -> int:
    from .mcp_server import run
    run()
    return 0


def cmd_home(args) -> int:
    """``panorama home --startup``: open the home tab once, without taking focus."""
    from . import home
    if not home.enabled():
        return 0
    _, err = home.ensure(focus=not args.startup)
    if err:
        print(f"panorama: home tab: {err}", file=sys.stderr)
        return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="panorama", description="Read-only swarm view of Claude Code agents in herdr.")
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("snapshot", help="print the derived model")
    s.add_argument("--json", action="store_true", help="full model as JSON")
    s.add_argument("--counts", action="store_true", help="numbers only, no content")
    s.set_defaults(fn=cmd_snapshot)
    s = sub.add_parser("tui", help="run the view")
    s.set_defaults(fn=cmd_tui)
    s = sub.add_parser("open", help="open the herdr plugin pane")
    s.add_argument("--split", action="store_true", help="side split instead of the popup")
    s.add_argument("--popup", action="store_true", help="the popup even when the home tab is on")
    s.set_defaults(fn=cmd_open)
    s = sub.add_parser("home", help="open or focus the home tab")
    s.add_argument("--startup", action="store_true", help="herdr startup hook: open once, do not focus")
    s.set_defaults(fn=cmd_home)
    s = sub.add_parser("hook", help="Claude Code hook entry point (plugin)")
    s.add_argument("kind")  # free text: an unknown kind is a no-op with exit 0, never argparse's exit 2
    s.set_defaults(fn=cmd_hook)
    s = sub.add_parser("mcp", help="MCP stdio server with report_thread (plugin)")
    s.set_defaults(fn=cmd_mcp)
    s = sub.add_parser("emit", help="declare or update a thread (explicit channel)")
    s.add_argument("--session", help="Claude session id (default: $CLAUDE_CODE_SESSION_ID)")
    s.add_argument("--thread", required=True)
    s.add_argument("--name")
    s.add_argument("--flavour")
    s.add_argument("--status", choices=["active", "done", "failed", "cancelled"])
    s.add_argument("--worktree")
    s.add_argument("--pct", type=int)
    s.add_argument("--phase", action="append", help='"name=state[@HH:MM[-HH:MM]][:note]", repeatable, replaces all phases')
    s.add_argument("--agent", action="append", help='"agentId|description[*][=role[@phase]]", repeatable')
    s.add_argument("--json", help="extra fields as a JSON object")
    s.set_defaults(fn=cmd_emit)
    args = p.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
