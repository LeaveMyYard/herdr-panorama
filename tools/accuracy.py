"""Accuracy gate: compare subagent statuses from zoe, the old scanner and the new one
against tools/check_status.py, per session, at one shared ``now``. Read-only.

    uv run python tools/accuracy.py <session-id-prefix>... [--old /path/to/old/src] [--details]

Statuses are compared in four classes: running, done, failed, stopped. Prints
counts and, with --details, the mismatching agent ids with both statuses (no
transcript content).
"""

from __future__ import annotations

import glob
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import check_status  # noqa: E402

PROJECTS = os.path.join(os.path.expanduser("~"), ".claude", "projects")
CLASS = {"run": "running", "rev": "running", "wait": "running", "blk": "running", "q": "running",
         "done": "done", "fail": "failed", "cxl": "stopped", "inf": "done", "lost": "stopped",
         "running": "running", "idle": "done", "failed": "failed", "stopped": "stopped"}
ZOE_LINE = re.compile(r"\[(subagent)\][^\n]*?\((\w+)\)\s+—\s+id=(\S+)")


def root_of(prefix: str) -> str:
    hits = glob.glob(os.path.join(PROJECTS, "*", f"{prefix}*.jsonl"))
    if len(hits) != 1:
        raise SystemExit(f"{prefix}: {len(hits)} matches")
    return hits[0]


def zoe(root: str) -> dict[str, str]:
    out = subprocess.run(["zoe", "inspect", root], capture_output=True, text=True, timeout=300).stdout
    return {m.group(3): CLASS.get(m.group(2), m.group(2)) for m in ZOE_LINE.finditer(out)}


SCANNER = r'''
import json, os, sys
sys.path.insert(0, sys.argv[1])
now = float(sys.argv[3]); root = sys.argv[2]
sid = os.path.basename(root)[:-6]
mode = sys.argv[4]
if mode == "old":
    from herdr_panorama.collect import Collector
    c = Collector(root=os.path.dirname(os.path.dirname(root)), now=lambda: now, done_window=1e12)
    snap = {"agents": [{"agent": "claude", "pane_id": "p1", "tab_id": "t1", "agent_status": "idle",
                        "agent_session": {"value": sid}, "cwd": None}], "tabs": [{"tab_id": "t1", "label": "x"}]}
    sw = c.collect(snap, None)
    print(json.dumps({a.id: a.status for o in sw.orchestrators for a in o.agents() if not a.id.startswith("main:")}))
else:
    from herdr_panorama.scan.session import Session
    s = Session(sid, root)
    s.update()
    print(json.dumps({aid: s.status_of(aid, now)[0] for aid, r in s.agents.items() if r.file}))
'''


def scanner(src: str, root: str, now: float, mode: str) -> dict[str, str]:
    p = subprocess.run([sys.executable, "-c", SCANNER, src, root, str(now), mode], capture_output=True, text=True,
                       timeout=600)
    if p.returncode:
        raise SystemExit(p.stderr[-2000:])
    return {k: CLASS.get(v, v) for k, v in json.loads(p.stdout).items()}


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    old_src = sys.argv[sys.argv.index("--old") + 1] if "--old" in sys.argv else None
    if old_src in args:
        args.remove(old_src)
    new_src = os.path.join(os.path.dirname(HERE), "src")
    details = "--details" in sys.argv
    for prefix in args:
        root = root_of(prefix)
        now = time.time()
        truth = check_status.check(root, now)
        results = {"zoe": zoe(root), "new": scanner(new_src, root, now, "new")}
        if old_src:
            results["old"] = scanner(old_src, root, now, "old")
        line = [f"{prefix}: {len(truth)} agents"]
        for name, got in results.items():
            agree = sum(1 for a, st in truth.items() if got.get(a) == st)
            line.append(f"{name} {agree}/{len(truth)}")
        print("  ".join(line))
        if details:
            for name, got in results.items():
                diffs = [(a, st, got.get(a, "missing")) for a, st in sorted(truth.items()) if got.get(a) != st]
                for a, st, g in diffs:
                    print(f"    {name:4} {a} truth={st} got={g}")


if __name__ == "__main__":
    main()
