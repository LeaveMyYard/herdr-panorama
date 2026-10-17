"""Capture the behaviour goldens of the Rust rewrite plan (docs/plans/rust-rewrite.md §5.2,
items 2 and 3) from the running Python code. Nothing in them is written by hand.

Every golden carries its inputs, so another implementation can replay it without Python:

- ``tests/fixtures/inputs/scenario-*.json``: each ``tests/scenario.py`` scenario as files
  (path relative to the scenario root, mtime, text), the herdr snapshot and the inbox rows.
- ``tests/fixtures/inputs/swarm-*.json``: the model-built swarms (``tests/demo.py``, the
  design datasets) in their wire form (``protocol.swarm_to_wire``).
- ``tests/fixtures/model/*.json`` (item 3): ``panorama snapshot --json`` (the real command,
  in its own process, ``time.time`` held) over every scenario at fixed ``now`` values and
  conditions; paths under the scenario root read ``<root>``.
- ``tests/fixtures/screens/*.json`` (item 2): the Textual apps headless (``run_test``), each
  file one or more ``runs`` (app, size, input, ``now``, environment) with an ordered list of
  ``steps``; every ``snap`` step's screen is under ``screens`` by its name, as runs of
  ``[text, fg, bg, bold]`` per row (the shape of ``design_*.json``; ``null`` is the
  terminal's default colour; a ``reverse`` cell has its fg and bg swapped, as the terminal
  shows it; no other attribute occurs on these screens).

The goldens are made from the input files (written first, then read back), so the inputs are
known to be complete. Deterministic: fixed sizes, held clock and animation frame, TZ=UTC, no
``HERDR_*``/``PANORAMA_*``/``TEXTUAL_*``/``CLAUDE_*``/``NO_COLOR`` from the caller, a fake herdr
binary, and none of the operator's transcripts, inbox, config or herdr. ``tests/test_goldens.py``
runs this into a temporary directory and compares. After a deliberate behaviour change:

    uv run python tools/capture_goldens.py              # rewrite tests/fixtures/{inputs,model,screens}/
    uv run python tools/capture_goldens.py --out DIR    # write DIR/... instead
"""

from __future__ import annotations

import argparse
import asyncio
import itertools
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
MIN = 60.0
HOUR = 3600.0
KINDS = ("inputs", "model", "screens")
_dirs = itertools.count()

# ``panorama snapshot --json`` with time.time held at argv[1]: patched before the package is
# imported, so the Collector's default clock is the held one too
FROZEN_SNAPSHOT = ("import sys, time\n"
                   "t = float(sys.argv[1])\n"
                   "time.time = lambda: t\n"
                   "from herdr_panorama.cli import main\n"
                   "sys.exit(main(['snapshot', '--json']))\n")

DROPPED_ENV = ("HERDR_", "PANORAMA_", "TEXTUAL_", "CLAUDE_")
DROPPED_VARS = ("NO_COLOR", "FORCE_COLOR", "COLORTERM", "TERM", "COLUMNS", "LINES")
ENV_NOTE = ("TZ=UTC; no herdr config.toml (panorama's default theme); NO_COLOR, FORCE_COLOR, COLORTERM, TERM and every "
            "HERDR_*/PANORAMA_*/TEXTUAL_*/CLAUDE_* variable unset except those a run names; "
            "tools/capture_goldens.py")


def isolate(work: Path) -> dict:
    """The process's environment: nothing of the caller's (conftest.no_real_state's rules)."""
    for key in [k for k in os.environ if k.startswith(DROPPED_ENV) or k in DROPPED_VARS]:
        del os.environ[key]
    os.environ.update({
        "TZ": "UTC",
        "HERDR_INBOX_DB": str(work / "no-inbox.db"),
        "PANORAMA_EVENTS_DIR": str(work / "no-events"),
        "HERDR_PLUGIN_CONFIG_DIR": str(work / "plugin-config"),
        "HERDR_PLUGIN_STATE_DIR": str(work / "plugin-state"),
        "PANORAMA_HOOKS_DIR": str(work / "hooks"),
        "XDG_CONFIG_HOME": str(work / "xdg-config"),
        "CLAUDE_CONFIG_DIR": str(work / "no-claude"),
    })
    time.tzset()
    sys.path.insert(0, str(ROOT))
    from tests.conftest import FakeHerdr
    fake = FakeHerdr(work)
    os.environ.update(HERDR_BIN_PATH=str(fake.bin), FAKE_HERDR_LOG=str(fake.log))
    return dict(os.environ)


def write_json(path: Path, doc: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")


def write_screens(path: Path, doc: dict) -> None:
    """One screen row per line: small diffs when behaviour changes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    parts = []
    for name, rows in doc["screens"].items():
        body = ",\n".join("    " + json.dumps(r, ensure_ascii=False, separators=(",", ":")) for r in rows)
        parts.append(f"  {json.dumps(name, ensure_ascii=False)}: [\n{body}\n  ]")
    head = json.dumps({k: v for k, v in doc.items() if k != "screens"}, indent=1, ensure_ascii=False)
    text = head[:-2] + ',\n "screens": {\n' + ",\n".join(parts) + "\n }\n}\n"
    json.loads(text)  # well-formed
    path.write_text(text)


# -- inputs ---------------------------------------------------------------------------------

def file_times(root: Path) -> dict[Path, float]:
    """Every transcript's mtime is its newest record's time (as if just written then); the
    undated files (metas, journals, run JSON, events) take the newest time of the scenario."""
    from datetime import datetime
    stamp = re.compile(r'"timestamp":"([^"]+)"')
    newest: dict[Path, float] = {}
    for p in root.rglob("*.jsonl"):
        ts = [datetime.fromisoformat(m.replace("Z", "+00:00")).timestamp() for m in stamp.findall(p.read_text())]
        if ts:
            newest[p] = max(ts)
    last = max(newest.values())
    return {p: newest.get(p, last) for p in sorted(root.rglob("*")) if p.is_file()}


def scenario_inputs(work: Path) -> dict[str, dict]:
    """Every tests/scenario.py scenario, as data."""
    from tests.scenario import SCENARIOS
    out = {}
    for name, build in SCENARIOS.items():
        base = work / "build" / name
        base.mkdir(parents=True)
        d = build(base)
        con = sqlite3.connect(d["inbox"])
        rows = [dict(zip(("id", "session", "pane", "asked_by", "question", "status"), r)) for r in con.execute(
            "SELECT id, session_id, pane_id, asked_by, question, status FROM questions ORDER BY id")]
        con.close()
        files = {str(p.relative_to(base)): {"mtime": t, "text": p.read_text()}
                 for p, t in file_times(base).items() if p != d["inbox"]}
        out[name] = {"source": f"tests/scenario.py {build.__name__}; mtimes: a transcript's newest record, "
                               "undated files the scenario's newest; tools/capture_goldens.py",
                     "now": d["now"], "projects": str(d["projects"].relative_to(base)),
                     "events": str(d["events"].relative_to(base)), "files": files, "snapshot": d["snapshot"],
                     "inbox": rows}
    return out


def materialize(inp: dict, base: Path) -> dict:
    """A scenario's files, snapshot and inbox database under ``base``; the env that points there."""
    from tests.scenario import inbox_db
    for rel, f in inp["files"].items():
        p = base / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(f["text"])
        os.utime(p, (f["mtime"], f["mtime"]))
    (base / inp["events"]).mkdir(parents=True, exist_ok=True)
    (base / "snapshot.json").write_text(json.dumps(inp["snapshot"]))
    db = inbox_db(base / "inbox.db", inp["inbox"])
    assert inp["projects"] == "projects"  # found as CLAUDE_CONFIG_DIR/projects
    return {"CLAUDE_CONFIG_DIR": str(base), "FAKE_HERDR_SNAPSHOT": str(base / "snapshot.json"),
            "HERDR_INBOX_DB": str(db), "PANORAMA_EVENTS_DIR": str(base / inp["events"])}


def swarm_inputs() -> dict[str, dict]:
    from herdr_panorama.protocol import swarm_to_wire
    from tests.demo import NOW, demo_swarm
    from tests.design_deep import NOW as DNOW, deep_swarm
    from tests.design_dock import FIX, design_swarm
    from tests.test_machines import mixed_swarm
    nine = json.loads((FIX / "design_9.json").read_text())["data"]
    made = {"demo": ("tests/demo.py demo_swarm", demo_swarm(), NOW),
            "deep": ("tests/design_deep.py deep_swarm", deep_swarm(), DNOW),
            "remote": ("tests/test_machines.py mixed_swarm (demo plus a remote machine's group)", mixed_swarm(), NOW),
            "design9": ("tests/design_dock.py design_swarm of design_9.json's data", design_swarm(nine), NOW)}
    return {k: {"source": src + " in protocol.swarm_to_wire form; now: the clock its tests use",
                "now": now, "swarm": swarm_to_wire(sw)} for k, (src, sw, now) in made.items()}


# -- item 3: the model ----------------------------------------------------------------------

MODEL_TIMES = {  # scenario -> offsets (minutes) from its own ``now``: live, later, past the done window, stale (12h)
    "demo": (0, 30, 3 * 60, 7 * 60, 13 * 60),
    "continued": (0, 2 * 60),
    "background": (0, 60),
}
CONDITIONS = {  # the demo again at its own now, with herdr or the inbox unusable
    "herdr down": {"FAKE_HERDR_FAIL": "api snapshot"},
    "no inbox": {"HERDR_INBOX_DB": "<root>/missing.db"},
}


def capture_model(out: Path, work: Path, inputs: dict, env0: dict) -> list[Path]:
    written = []
    for name, inp in inputs.items():
        base = work / "model" / name
        base.mkdir(parents=True)
        env = {**env0, **materialize(inp, base)}
        runs = [(f"+{off}m", inp["now"] + off * MIN, {}) for off in MODEL_TIMES[name]]
        if name == "demo":
            runs += [(f"+0m {cond}", inp["now"], extra) for cond, extra in CONDITIONS.items()]
        snaps, conds = {}, {}
        for key, now, extra in runs:
            res = subprocess.run([sys.executable, "-c", FROZEN_SNAPSHOT, repr(now)], cwd=base, timeout=120,
                                 env={**env, **{k: v.replace("<root>", str(base)) for k, v in extra.items()}},
                                 capture_output=True, text=True)
            if res.returncode != 0:
                raise SystemExit(f"panorama snapshot --json failed ({name} {key}):\n{res.stderr}")
            snaps[key] = json.loads(res.stdout.replace(str(base), "<root>"))
            if extra:
                conds[key] = extra
        doc = {"source": f"panorama snapshot --json over inputs/scenario-{name}.json materialised under <root> "
                         "(CLAUDE_CONFIG_DIR=<root>, HERDR_INBOX_DB=<root>/inbox.db from the inbox rows, "
                         "PANORAMA_EVENTS_DIR=<root>/events, a fake herdr answering `api snapshot` with the "
                         "input's snapshot and an empty `pane process-info`), time.time held at the scenario's "
                         "now plus each key's minutes, the conditions' env added; " + ENV_NOTE,
               "input": f"inputs/scenario-{name}.json", "now": inp["now"],
               "conditions": conds, "snapshots": snaps}
        path = out / "model" / f"{name}.json"
        write_json(path, doc)
        written.append(path)
    return written


# -- item 2: the app screens ----------------------------------------------------------------

def press(*keys): return {"do": "press", "keys": list(keys)}
def frame(n=0): return {"do": "frame", "n": n}
def snap(name): return {"do": "snap", "name": name}
def disarm(): return {"do": "disarm"}
def typed(text): return {"do": "type", "text": text}


def screen_files(scenarios: dict, swarms: dict) -> dict[str, list[dict]]:
    """file -> runs. A run: app, size, input, now, focus_pane, modes, docked, env; then
    ``steps``, each followed by a Textual pause: ``press`` keys; ``frame`` n (hold the
    animation at design frame n and repaint); ``snap`` (the screen, by name); ``disarm`` (the
    arming window has passed); ``hover`` a target; ``blur`` (the terminal lost focus);
    ``select`` thread/agent/focus column; ``type`` text (one key per character); ``set`` a
    widget's value; ``state`` (a panel: the core's state message with that swarm); ``core`` (a
    panel: the core connected or gone)."""
    demo_now, deep_now = swarms["demo"]["now"], swarms["deep"]["now"]
    keys = [press("down"), frame(), snap("key 1 down"), press("right"), frame(), snap("key 2 right"),
            press("down"), frame(), press("right"), frame(), snap("key 4 right")]  # key 3: as key 2
    popup = {"app": "swarm", "focus_pane": "w1:p1", "home_mode": False, "cockpit_mode": False, "docked": None}
    form = [press("n"), {"do": "set", "selector": "#cwd", "value": "/home/user/work/billing"},
            typed("Billing export"), press("tab", "tab"), typed("Add CSV export to the invoices page")]
    files = {
        "demo-wide": [{**popup, "size": [160, 45], "input": "swarm-demo", "now": demo_now,
                       "env": {"FAKE_HERDR_FAIL": "agent focus"}, "steps": [
            frame(), snap("initial"), frame(4), snap("initial@frame4"), frame(), *keys,
            press("f"), frame(), snap("filter running"), press("f"), frame(), snap("filter needs"),
            press("f"), frame(), press("question_mark"), snap("help toast"),
            disarm(), press("t"), snap("transcript toast"),
            disarm(), press("g"), snap("go-to failed toast")]}],
        "demo-narrow": [{**popup, "size": [70, 45], "input": "swarm-demo", "now": demo_now,
                         "steps": [frame(), snap("initial"), *keys]}],
        "demo-home": [{**popup, "size": [160, 45], "input": "swarm-demo", "now": demo_now, "home_mode": True,
                       "steps": [frame(), snap("home"), *form, snap("new agent form")]}],
        "new-agent-popup": [{"app": "new_agent", "size": [84, 26],
                             "env": {"PANORAMA_NEW_AGENT_CWD": "/home/user/work/billing"},
                             "steps": [snap("empty"), *form[2:], snap("typed")]}],
        "dock": [{**popup, "size": [46, 45], "input": "swarm-design9", "now": demo_now, "focus_pane": "w1:p3",
                  "home_mode": True, "cockpit_mode": True, "docked": "w1:p3", "steps": [
            frame(), snap("focused"), {"do": "hover", "target": ["orch", ["w1:p4"]]}, frame(4),
            snap("hover@frame4"), {"do": "hover", "target": None}, press("down"), frame(), snap("key down"),
            {"do": "blur"}, frame(), snap("unfocused")]}],
        "deep": [{**popup, "size": [160, 45], "input": "swarm-deep", "now": deep_now, "focus_pane": "w1:p3",
                  "steps": [{"do": "select", "thread": "x:deep", "agent": None, "focus": 1}, frame(), snap("thread"),
                            {"do": "select", "thread": "x:deep", "agent": "c1", "focus": 5}, frame(), snap("c1")]}],
        "remote-wide": [{**popup, "size": [160, 45], "input": "swarm-remote", "now": demo_now, "home_mode": True,
                         "steps": [frame(), snap("initial")]}],
        "remote-dock": [{**popup, "size": [46, 45], "input": "swarm-remote", "now": demo_now, "home_mode": True,
                         "cockpit_mode": True, "docked": "w1:p1", "steps": [frame(), snap("initial")]}],
        "panel": [{"app": "panel", "size": [46, 45], "now": demo_now, "pane_id": "w1:p92", "tab_id": "w1:t2",
                   "steps": [
            snap("before the first state"),
            {"do": "core", "connected": True},
            {"do": "state", "swarm": "swarm-design9", "visible": ["w1:t2"]}, frame(), snap("shown"),
            {"do": "state", "swarm": "swarm-design9", "visible": ["w1:t2"], "filter": "running"}, frame(),
            snap("filter running from the core"),
            {"do": "core", "connected": False}, frame(), snap("no core")]}],
    }
    for name, inp in scenarios.items():
        focus = inp["snapshot"]["agents"][0]["pane_id"]
        base = {**popup, "input": f"scenario-{name}", "now": inp["now"], "focus_pane": focus}
        files[f"scenario-{name}-wide"] = [{**base, "size": [160, 45], "steps": [
            frame(), snap("initial"), press("right"), frame(), snap("key 1 right")]}]
        files[f"scenario-{name}-narrow"] = [{**base, "size": [70, 45], "steps": [frame(), snap("initial")]}]
        files[f"scenario-{name}-dock"] = [{**base, "size": [46, 45], "home_mode": True, "cockpit_mode": True,
                                           "docked": focus, "steps": [frame(), snap("initial"), press("down"),
                                                                      frame(), snap("key down")]}]
    # idle ages and staleness at their boundaries (render.ago, render.is_stale), and the failure
    # states. Release notes is idle since its last record; the other tabs for less long.
    rel = next(f["mtime"] for k, f in scenarios["demo"]["files"].items()
               if k.endswith("44444444-4444-4444-8444-444444444444.jsonl"))
    demo = {**popup, "size": [160, 45], "input": "scenario-demo"}
    files["scenario-demo-idle"] = [
        {**demo, "now": rel + 12 * HOUR + MIN, "steps": [frame(), snap("release notes idle 12h01m (stale_after 12h)")]},
        {**demo, "now": rel + 30 * HOUR, "steps": [frame(), snap("release notes idle 30h")]},
        {**demo, "now": rel + 48 * HOUR + MIN, "steps": [frame(), snap("release notes idle 48h01m, billing 47h")]},
        {**demo, "now": rel + 59 * MIN + 30, "env": {"PANORAMA_STALE_AFTER": "30m"},
         "steps": [frame(), snap("stale_after 30m, release notes idle 59m")]},
        {**demo, "now": rel + HOUR + 30, "env": {"PANORAMA_STALE_AFTER": "30m"},
         "steps": [frame(), snap("stale_after 30m, release notes idle 60m")]},
    ]
    files["scenario-demo-failures"] = [
        {**demo, "now": scenarios["demo"]["now"], "env": {"FAKE_HERDR_FAIL": "api snapshot"},
         "steps": [frame(), snap("herdr down")]},
        {**demo, "now": scenarios["demo"]["now"], "env": {"HERDR_INBOX_DB": "<root>/missing.db"},
         "steps": [frame(), snap("no inbox")]},
    ]
    return files


def hexcolor(c) -> str | None:
    if c is None or c.is_default:
        return None
    return c.get_truecolor().hex.lower()


def dump(app) -> list[list]:
    """The screen as the terminal gets it (export_screenshot's own render), runs per row."""
    update = app.screen._compositor.render_update(full=True, screen_stack=app._background_screens)
    rows = []
    for strips in update.strips:
        row: list[list] = []
        for strip in strips:
            for seg in strip:
                if seg.control or not seg.text:
                    continue
                st = seg.style
                fg, bg = (hexcolor(st.color), hexcolor(st.bgcolor)) if st else (None, None)
                if st and st.reverse:  # what the terminal shows: a text cursor, a selection
                    fg, bg = bg, fg
                attrs = (fg, bg, bool(st and st.bold))
                if row and tuple(row[-1][1:]) == attrs:
                    row[-1][0] += seg.text
                else:
                    row.append([seg.text, *attrs])
        rows.append(row)
    return rows


async def settle(app, pilot, wait_loaded: bool) -> None:
    """The first model has painted; fails loudly rather than capture a half-loaded screen."""
    deadline = time.monotonic() + 30
    while wait_loaded and not app.loaded:
        if time.monotonic() > deadline:
            raise SystemExit("the app never loaded its first model")
        await pilot.pause(0.02)
    await pilot.pause()


def hold_cursor(app) -> None:
    """A text input's cursor blinks on a wall-clock timer: held on."""
    from textual.widgets import Input
    for w in app.screen.query(Input):
        w.cursor_blink = False


async def play(run: dict, inputs: dict, work: Path, env0: dict, shots: dict) -> None:
    from textual import events

    from herdr_panorama import herdr, inbox, protocol
    from herdr_panorama.collect import Collector
    from herdr_panorama.panel import PanelApp
    from herdr_panorama.tui import NewAgentApp, SwarmApp
    from tests.anim import at_frame

    base = work / "screens" / str(next(_dirs))
    base.mkdir(parents=True)
    os.environ.clear()
    os.environ.update(env0)
    inp, collector, source = run.get("input"), None, None
    if inp and inp.startswith("scenario-"):
        os.environ.update(materialize(inputs[inp], base))
        collector = Collector(now=lambda: run["now"])

        def source():  # the view's own default source (herdr + the inbox), its clock held
            sw = collector.collect(herdr.snapshot(), inbox.open_questions())
            sw.docked = run["docked"]
            return sw
    elif inp:
        def source():
            sw = protocol.swarm_from_wire(inputs[inp]["swarm"])
            sw.docked = run["docked"]
            return sw
    os.environ.update({k: v.replace("<root>", str(base)) for k, v in (run.get("env") or {}).items()})
    if run["app"] == "swarm":
        app = SwarmApp(source=source, collector=collector, live_refresh=False, clock=lambda: run["now"],
                       focus_pane=run["focus_pane"], home_mode=run["home_mode"], cockpit_mode=run["cockpit_mode"])
    elif run["app"] == "new_agent":
        app = NewAgentApp()
    else:
        app = PanelApp(sock_path=base / "core.sock", pane_id=run["pane_id"], tab_id=run["tab_id"],
                       clock=lambda: run["now"], start_client=False)
    async with app.run_test(size=tuple(run["size"]), notifications=True) as pilot:
        await settle(app, pilot, run["app"] == "swarm")
        hold_cursor(app)
        for step in run["steps"]:
            do = step["do"]
            if do == "press":
                await pilot.press(*step["keys"])
            elif do == "type":
                await pilot.press(*step["text"])
            elif do == "set":
                app.screen.query_one(step["selector"]).value = step["value"]
            elif do == "frame":
                at_frame(app, step["n"])
                app.repaint()
            elif do == "disarm":
                app.armed_at = 0.0
            elif do == "hover":
                app.view.hover = (step["target"][0], tuple(step["target"][1])) if step["target"] else None
            elif do == "blur":
                app.post_message(events.AppBlur())
            elif do == "select":
                app.view.thread, app.view.agent, app.view.focus = step["thread"], step["agent"], step["focus"]
            elif do == "state":
                st = {"protocol": protocol.PROTOCOL, "swarm": inputs[step["swarm"]]["swarm"], "unseen": {},
                      "focused_pane": step.get("focused"), "visible_tabs": step.get("visible"), "panels": {}}
                if "filter" in step:
                    st["filter"] = step["filter"]
                app._got_state(st)
            elif do == "core":
                app._status(step["connected"])
            elif do != "snap":
                raise SystemExit(f"unknown step {step}")
            await pilot.pause()
            hold_cursor(app)
            if do == "snap":
                assert step["name"] not in shots, step["name"]
                shots[step["name"]] = dump(app)


def capture_screens(out: Path, work: Path, inputs: dict, env0: dict) -> list[Path]:
    from herdr_panorama import cockpit
    flat = {**{f"scenario-{k}": v for k, v in inputs["scenarios"].items()},
            **{f"swarm-{k}": v for k, v in inputs["swarms"].items()}}
    written = []
    for name, runs in screen_files(inputs["scenarios"], inputs["swarms"]).items():
        shots: dict = {}
        for run in runs:
            cockpit.moving_until = 0.0
            asyncio.run(play(run, flat, work, env0, shots))
        os.environ.clear()
        os.environ.update(env0)
        for run in runs:
            if run.get("input"):
                run["input"] = f"inputs/{run['input']}.json"
            for step in run["steps"]:
                if "swarm" in step:
                    step["swarm"] = f"inputs/{step['swarm']}.json"
        doc = {"source": "Textual run_test(notifications=True) of each run's app; a scenario input is "
                         "materialised as for model/ and read through the app's own source (herdr + the "
                         "inbox) with the clock held; a swarm input through protocol.swarm_from_wire; "
                         "<root> in env: that run's own directory; " + ENV_NOTE,
               "runs": runs, "screens": shots}
        path = out / "screens" / f"{name}.json"
        write_screens(path, doc)
        written.append(path)
    return written


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--out", type=Path, default=FIXTURES, help="write OUT/{inputs,model,screens} (default: %(default)s)")
    p.add_argument("--only", choices=("model", "screens"), help="one of the two golden sets (the inputs always)")
    args = p.parse_args(argv)
    work = Path(tempfile.mkdtemp(prefix="panorama-goldens-"))
    try:
        env0 = isolate(work)
        for kind in KINDS:
            if args.only in (None, kind) or kind == "inputs":
                shutil.rmtree(args.out / kind, ignore_errors=True)
        written = []
        for name, inp in scenario_inputs(work).items():
            written.append(args.out / "inputs" / f"scenario-{name}.json")
            write_json(written[-1], inp)
        for name, inp in swarm_inputs().items():
            written.append(args.out / "inputs" / f"swarm-{name}.json")
            write_json(written[-1], inp)
        # the goldens are made from the files just written: the inputs are complete
        inputs: dict = {"scenarios": {}, "swarms": {}}
        for f in sorted((args.out / "inputs").glob("*.json")):
            kind, _, name = f.stem.partition("-")
            inputs["scenarios" if kind == "scenario" else "swarms"][name] = json.loads(f.read_text())
        if args.only in (None, "model"):
            written += capture_model(args.out, work, inputs["scenarios"], env0)
        if args.only in (None, "screens"):
            written += capture_screens(args.out, work, inputs, env0)
        size = sum(f.stat().st_size for f in written)
        print(f"{len(written)} files, {size // 1024} KiB under {args.out}")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
