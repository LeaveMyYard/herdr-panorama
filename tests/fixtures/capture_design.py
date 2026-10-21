"""Capture the design references: the sample dataset, turn 5's direction 5d (subagents to any
depth: its dataset and its frames), turn 6 (unseen), turn 7 (wider cards), turn 8 (waiting and stale),
turn 9's 9e (tiles in the docked panel), turn 10 (the panel focused or not, in herdr's theme), the
smooth animation (turn 6 between whole frames) and the new-agent modal (design 1C "Coming soon").

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

WIDE_JS = """
import { TREE, THREAD } from './swarm-wide.js';
const flat = (a) => ({ id: a.hex, role: a.role, st: a.st, act: a.act, model: a.model, age: a.age, tools: a.tools,
  n: a._n, q: a.q || 0, qs: a.qs || [], err: a.err || null, kids: a.kids.map(flat) });
console.log(JSON.stringify({ thread: { name: THREAD.name, ph: THREAD.ph.map(p => [p[0], p[1]]) }, tree: TREE.map(flat) }));
"""


STATES_JS = """
import { STATES } from './swarm-states.js';
const n1 = STATES.DATA.flatMap(o => o.threads).flatMap(t => t.agents).find(a => a.id === 'n1');
console.log(JSON.stringify({
  orchs: STATES.DATA.map(o => ({ name: o.name, tab: o.tab, fl: o.fl, last: o.last, bgAge: o.bgAge || null,
    bg: (o.bg || []).map(b => ({ kind: b.kind, desc: b.desc, since: b.since })) })),
  n1: { st: n1.st, act: n1.act, msg: n1.msg, phs: n1.phs, tools: n1.tools, age: n1.age,
    bg: n1.bg.map(b => ({ kind: b.kind, desc: b.desc, since: b.since })) } }));
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
    nine = json.loads(node("render-9.mjs", "json"))
    nine["frames"] = {f: {k: [run_length(r) for r in v] for k, v in fr.items()} for f, fr in nine["frames"].items()}
    nine["source"] = ("docs/design/swarm-tiles.js (turn 9, variant 9e) rendered by docs/design/render-9.mjs: per frame "
                      "E the docked panel, U the same with turn 6's unseen sample marks; data the dataset (swarm-tui.js "
                      "plus the finished Docs Site); rows as runs of [text, fg, bg, bold]")
    (HERE / "design_9.json").write_text(json.dumps(nine, ensure_ascii=False, separators=(",", ":")))
    ten = json.loads(node("render-10.mjs", "json"))
    ten["frames"] = {f: {k: [run_length(r) for r in v] for k, v in fr.items()} for f, fr in ten["frames"].items()}
    ten["source"] = ("docs/design/swarm-focus.js (turn 10) rendered by docs/design/render-10.mjs: the 9e panel focused "
                     "(A) and not (I) in two herdr themes (themes: the tokens used); rows as runs of [text, fg, bg, bold]")
    (HERE / "design_10.json").write_text(json.dumps(ten, ensure_ascii=False, separators=(",", ":")))
    smooth = {f: {k: [run_length(r) for r in v] for k, v in fr.items()}
              for f, fr in json.loads(node("render-smooth.mjs", "json")).items()}
    (HERE / "design_smooth.json").write_text(json.dumps(
        {"source": "docs/design/swarm-unseen.js with swarm-smooth.js (smooth animation) rendered by "
                   "docs/design/render-smooth.mjs between whole frames: G and N as in design_6.json, E and U (swarm-tiles.js 9e) as in design_9.json",
         "frames": smooth}, ensure_ascii=False, separators=(",", ":")))
    wide = json.loads(node("--input-type=module", "-", stdin=WIDE_JS))
    wide["frames"] = {f: {k: [run_length(r) for r in v] for k, v in json.loads(node("render-7.mjs", str(f))).items()
                          if k != "frame"} for f in (4, 9)}
    wide["source"] = ("docs/design/swarm-wide.js (turn 7, wider cards) rendered by docs/design/render-7.mjs: W160/W240/"
                      "W320 focus on the thread, R160/R320 on depth 2, S the sweep strip; rows as runs of [text, fg, bg, bold]")
    (HERE / "design_7.json").write_text(json.dumps(wide, ensure_ascii=False, separators=(",", ":")))
    eight = {f: {k: [run_length(r) for r in v] for k, v in fr.items()}
             for f, fr in json.loads(node("render-8.mjs", "json")).items()}
    (HERE / "design_8.json").write_text(json.dumps(
        {"source": "docs/design/swarm-states.js (turn 8, waiting and stale) rendered by docs/design/render-8.mjs: per "
                   "frame A the 2a graph (Deploy Watch's background card), B the 2a graph (n1 waiting), R the same "
                   "with the running filter, P the 46 docked-panel columns; rows as runs of [text, fg, bg, bold]",
         "data": json.loads(node("--input-type=module", "-", stdin=STATES_JS)), "frames": eight},
        ensure_ascii=False, separators=(",", ":")))

    new = json.loads(node("render-new-agent.mjs"))
    for fr in new["frames"]:
        fr["rows"] = [run_length(r) for r in fr["rows"]]
    new["input"] = {k: [run_length(r) for r in v] for k, v in new["input"].items()}
    new["source"] = ("docs/design/herdr-tui.js (Claude Design project aae25d19, `New agent + empty inbox`, card "
                     "1C-soon) rendered by docs/design/render-new-agent.mjs: frames: modalCSoon(sel) per harness, "
                     "96x32 with the modal 76x27 at (10, 3), and its click targets; input: the design's input() "
                     "focused with its cursor on a 76x6 panel at x 3; rows as runs of [text, fg, bg, bold]")
    (HERE / "design_new_agent.json").write_text(json.dumps(new, ensure_ascii=False, separators=(",", ":")))


if __name__ == "__main__":
    main()
