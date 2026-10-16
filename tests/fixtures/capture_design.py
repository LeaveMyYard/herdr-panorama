"""Capture the design references: the sample dataset, the 3a dock panel render, turn 5's
direction 5d (subagents to any depth: its dataset and its frames) and turn 6 (unseen).

    uv run python tests/fixtures/capture_design.py   (needs node)
"""

import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
DESIGN = HERE.parent.parent / "docs" / "design"

DATA_JS = """
import { DATA } from './swarm-tui.js';
console.log(JSON.stringify(DATA.map(o => ({ name: o.name, tab: o.tab, fl: o.fl, threads: o.threads.map(t => ({
  id: t.id, name: t.name, ph: t.ph.map(p => [p[0], p[1]]),
  agents: t.agents.map(a => ({ id: a.id, role: a.role, model: a.model, st: a.st, q: a.q || 0, n: a._n,
    act: a.act, msg: a.msg || '', qs: a.qs || [], err: a.err || null, perm: a.perm || null })) })) }))));
"""

DEEP_JS = """
import { TREE, THREAD } from './swarm-deep.js';
const flat = (a) => ({ id: a.id, role: a.role, st: a.st, act: a.act, model: a.model, age: a.age, tools: a.tools,
  n: a._n, q: a.q || 0, qs: a.qs || [], err: a.err || null, kids: a.kids.map(flat) });
console.log(JSON.stringify({ thread: { name: THREAD.name, ph: THREAD.ph.map(p => [p[0], p[1]]) }, tree: TREE.map(flat) }));
"""


def node(*args: str, stdin: str | None = None) -> str:
    return subprocess.run(["node", *args], cwd=DESIGN, input=stdin, capture_output=True, text=True, check=True).stdout


def run_length(cells: list) -> list:
    out: list = []
    for ch, fg, bg, b in cells:
        if out and out[-1][1:] == [fg, bg, b]:
            out[-1][0] += ch
        else:
            out.append([ch, fg, bg, b])
    return out


def main() -> None:
    (HERE / "design_data.json").write_text(node("--input-type=module", "-", stdin=DATA_JS))
    frames = {}
    for f in (0, 1, 2, 3, 4, 7):
        rows = json.loads(node("render-3a.mjs", str(f), "json"))["rows"]
        out = []
        for r in rows:
            runs: list = []
            for ch, fg, bg, b in r:
                if runs and runs[-1][1:] == [fg, bg, b]:
                    runs[-1][0] += ch
                else:
                    runs.append([ch, fg, bg, b])
            out.append(runs)
        frames[f] = out
    deep = json.loads(node("--input-type=module", "-", stdin=DEEP_JS))
    deep["frames"] = {f: {k: [run_length(r) for r in v] for k, v in json.loads(node("render-5d.mjs", str(f), "json")).items()
                          if k != "frame"} for f in (0, 1, 4)}
    deep["source"] = ("docs/design/swarm-deep.js direction 5d rendered by docs/design/render-5d.mjs: V4 focus on the "
                      "thread, V5 on b1 (depth 2), V6 on c1 (depth 4), P4 the docked panel; rows as runs of [text, fg, bg, bold]")
    (HERE / "design_5d.json").write_text(json.dumps(deep, ensure_ascii=False, separators=(",", ":")))
    six = {f: {k: [run_length(r) for r in v] for k, v in fr.items()}
           for f, fr in json.loads(node("render-6.mjs", "json")).items()}
    (HERE / "design_6.json").write_text(json.dumps(
        {"source": "docs/design/swarm-unseen.js (turn 6, unseen) rendered by docs/design/render-6.mjs: per frame G the "
                   "2a graph with the sample unseen marks, N the same with none, P the 46 docked-panel columns; rows "
                   "as runs of [text, fg, bg, bold]", "frames": six}, ensure_ascii=False, separators=(",", ":")))
    (HERE / "design_3a.json").write_text(json.dumps(
        {"source": "docs/design/swarm-dock.js vA (3a) rendered by docs/design/render-3a.mjs; the 46 panel columns "
                   "as runs of [text, fg, bg, bold]", "frames": frames}, ensure_ascii=False, separators=(",", ":")))


if __name__ == "__main__":
    main()
