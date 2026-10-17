"""Capture the behaviour goldens of the Rust rewrite plan (docs/plans/rust-rewrite.md §5.2,
items 2 and 3) from the running Python code. Nothing in them is written by hand.

- ``tests/fixtures/screens/*.json`` (item 2, app-screen goldens): the Textual apps run headless
  (``run_test``) over the test scenarios; every screen is what Textual's compositor sends to
  the terminal, as runs of ``[text, fg, bg, bold]`` per row (the shape of ``design_*.json``;
  ``null`` is the terminal's default colour).
- ``tests/fixtures/model/*.json`` (item 3, model goldens): ``panorama snapshot --json`` (the
  real command, in its own process) over every ``tests/scenario.py`` scenario at fixed ``now``
  values. Paths under the scenario's directory read ``<root>``.

Deterministic: fixed sizes, a held clock (``now``) and animation frame, TZ=UTC, every
``HERDR_*`` variable dropped, the herdr binary a fake that serves the scenario's snapshot, and
the operator's own transcripts, inbox, config and herdr never read. ``tests/test_goldens.py``
runs this into a temporary directory and compares. Regenerate after a deliberate behaviour
change (and say why in the PR):

    uv run python tools/capture_goldens.py              # rewrite tests/fixtures/{screens,model}/
    uv run python tools/capture_goldens.py --out DIR    # write DIR/{screens,model}/ instead
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
MIN = 60.0

# ``panorama snapshot --json`` with time.time held at argv[1]: patched before the package is
# imported, so the Collector's default clock is the held one too
FROZEN_SNAPSHOT = ("import sys, time\n"
                   "t = float(sys.argv[1])\n"
                   "time.time = lambda: t\n"
                   "from herdr_panorama.cli import main\n"
                   "sys.exit(main(['snapshot', '--json']))\n")


def isolate(work: Path) -> None:
    """The process's environment: nothing of the operator's (conftest.no_real_state's rules)."""
    for key in [k for k in os.environ if k.startswith("HERDR_") or k.startswith("PANORAMA_")]:
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
        "PANORAMA_NEW_AGENT_CWD": "/home/user/work/billing",
    })
    time.tzset()
    sys.path.insert(0, str(ROOT))


def write(path: Path, doc: dict) -> None:
    """One screen row (or one model line) per line: small diffs when behaviour changes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    if "screens" in doc:
        parts = []
        for name, rows in doc["screens"].items():
            body = ",\n".join("    " + json.dumps(r, ensure_ascii=False, separators=(",", ":")) for r in rows)
            parts.append(f"  {json.dumps(name)}: [\n{body}\n  ]")
        head = {k: v for k, v in doc.items() if k != "screens"}
        text = json.dumps(head, ensure_ascii=False)[:-1] + ', "screens": {\n' + ",\n".join(parts) + "\n}}\n"
        json.loads(text)  # well-formed
    else:
        text = json.dumps(doc, indent=1, ensure_ascii=False) + "\n"
    path.write_text(text)


# -- item 3: the model --------------------------------------------------------------------

def file_times(root: Path) -> None:
    """Every transcript's mtime is its newest record's time (as if just written then); the
    undated files (metas, journals, run JSON) take the newest time of the whole scenario."""
    stamp = re.compile(r'"timestamp":"([^"]+)"')
    from datetime import datetime
    newest: dict[Path, float] = {}
    for p in root.rglob("*.jsonl"):
        ts = [datetime.fromisoformat(m.replace("Z", "+00:00")).timestamp() for m in stamp.findall(p.read_text())]
        if ts:
            newest[p] = max(ts)
    last = max(newest.values())
    for p in root.rglob("*"):
        if p.is_file():
            t = newest.get(p, last)
            os.utime(p, (t, t))


MODEL_TIMES = {  # scenario -> offsets from its own ``now``: live, later, past the done window, stale (12h)
    "demo": (0, 30 * MIN, 3 * 60 * MIN, 7 * 60 * MIN, 13 * 60 * MIN),
    "continued": (0, 2 * 60 * MIN),
    "background": (0, 60 * MIN),
}


def capture_model(out: Path, work: Path) -> list[Path]:
    from tests.conftest import FakeHerdr
    from tests.scenario import SCENARIOS
    written = []
    for name, build in SCENARIOS.items():
        base = work / "model" / name
        base.mkdir(parents=True)
        d = build(base)
        file_times(d["projects"])
        fake = FakeHerdr(base)
        (base / "snapshot.json").write_text(json.dumps(d["snapshot"]))
        env = {**os.environ, "CLAUDE_CONFIG_DIR": str(base), "HERDR_BIN_PATH": str(fake.bin),
               "FAKE_HERDR_LOG": str(fake.log), "FAKE_HERDR_SNAPSHOT": str(base / "snapshot.json"),
               "HERDR_INBOX_DB": str(d["inbox"]), "PANORAMA_EVENTS_DIR": str(d["events"])}
        snaps = {}
        for off in MODEL_TIMES[name]:
            now = d["now"] + off
            res = subprocess.run([sys.executable, "-c", FROZEN_SNAPSHOT, repr(now)], env=env, cwd=base,
                                 capture_output=True, text=True, timeout=120)
            if res.returncode != 0:
                raise SystemExit(f"panorama snapshot --json failed ({name}, now={now}):\n{res.stderr}")
            snaps[f"+{int(off // MIN)}m"] = json.loads(res.stdout.replace(str(base), "<root>"))
        doc = {"source": "panorama snapshot --json over tests/scenario.py "
                         f"{build.__name__}, at its now ({d['now']}) plus each key's minutes; TZ=UTC; "
                         "tools/capture_goldens.py",
               "now": d["now"], "snapshots": snaps}
        path = out / "model" / f"{name}.json"
        write(path, doc)
        written.append(path)
    return written


# -- item 2: the app screens --------------------------------------------------------------

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
                attrs = (hexcolor(st.color) if st else None, hexcolor(st.bgcolor) if st else None,
                         bool(st and st.bold))
                if row and tuple(row[-1][1:]) == attrs:
                    row[-1][0] += seg.text
                else:
                    row.append([seg.text, *attrs])
        rows.append(row)
    return rows


async def settle(app, pilot) -> None:
    for _ in range(100):
        await pilot.pause(0.02)
        if getattr(app, "loaded", True):
            break
    await pilot.pause()


async def frame(app, pilot, n: float = 0) -> None:
    from tests.anim import at_frame
    at_frame(app, n)
    app.repaint()
    await pilot.pause()


def no_blink(app) -> None:
    """A text input's cursor blinks on a wall-clock timer: held on."""
    from textual.widgets import Input
    for w in app.screen.query(Input):
        w.cursor_blink = False


class Shots:
    def __init__(self):
        self.files: dict[str, dict] = {}

    def add(self, file: str, source: str, size: tuple[int, int], name: str, app) -> None:
        doc = self.files.setdefault(file, {"source": source, "size": list(size), "screens": {}})
        assert doc["size"] == list(size) and name not in doc["screens"], (file, name)
        doc["screens"][name] = dump(app)


async def swarm_screens(shots: Shots) -> None:
    from textual import events

    from herdr_panorama.tui import NewAgentApp, SwarmApp
    from tests.demo import NOW, demo_swarm
    from tests.design_deep import NOW as DNOW, TID, deep_swarm
    from tests.design_dock import FIX, design_swarm
    from tests.test_machines import mixed_swarm

    src = "tests/demo.py demo_swarm, SwarmApp (the popup view)"
    for size, file in (((160, 45), "demo-wide"), ((70, 45), "demo-narrow")):
        app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1")
        async with app.run_test(size=size) as pilot:
            await settle(app, pilot)
            await frame(app, pilot, 0)
            shots.add(file, src, size, "initial", app)
            if file == "demo-wide":
                await frame(app, pilot, 4)
                shots.add(file, src, size, "initial@frame4", app)
                await frame(app, pilot, 0)
            keep = (1, 2, 4) if file == "demo-wide" else (2, 4)
            for i, k in enumerate(("down", "right", "down", "right"), 1):
                await pilot.press(k)
                await frame(app, pilot, 0)
                if i in keep:
                    shots.add(file, src, size, f"key {i} {k}", app)
            if file == "demo-wide":
                for i in (1, 2):
                    await pilot.press("f")
                    await frame(app, pilot, 0)
                    shots.add(file, src, size, f"filter {i}", app)

    src = "tests/demo.py demo_swarm, SwarmApp home_mode (the home tab)"
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await settle(app, pilot)
        await frame(app, pilot, 0)
        shots.add("demo-home", src, (160, 45), "home", app)
        await pilot.press("n")
        await pilot.pause()
        no_blink(app)
        app.screen.query_one("#cwd").value = "/home/user/work/billing"
        await pilot.press(*"Billing export")
        await pilot.press("tab", "tab")
        await pilot.press(*"Add CSV export to the invoices page")
        await pilot.pause()
        shots.add("demo-home", src, (160, 45), "new agent form", app)

    src = "SwarmApp's NewAgentApp (panorama new-agent, the form as its own popup)"
    app = NewAgentApp()
    async with app.run_test(size=(84, 26)) as pilot:
        await pilot.pause()
        no_blink(app)
        await pilot.pause()
        shots.add("new-agent-popup", src, (84, 26), "empty", app)
        await pilot.press(*"Billing export")
        await pilot.press("tab", "tab")
        await pilot.press(*"Add CSV export to the invoices page")
        await pilot.pause()
        shots.add("new-agent-popup", src, (84, 26), "typed", app)

    nine = json.loads((FIX / "design_9.json").read_text())["data"]
    src = "tests/design_dock.py design_swarm (design_9.json data), SwarmApp cockpit_mode (the docked panel)"
    app = SwarmApp(source=lambda: design_swarm(nine), live_refresh=False, clock=lambda: NOW, focus_pane="w1:p3",
                   home_mode=True, cockpit_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        await settle(app, pilot)
        await frame(app, pilot, 0)
        shots.add("dock", src, (46, 45), "focused", app)
        app.view.hover = ("orch", ("w1:p4",))
        await frame(app, pilot, 4)
        shots.add("dock", src, (46, 45), "hover@frame4", app)
        app.view.hover = None
        app.post_message(events.AppBlur())
        await frame(app, pilot, 0)
        shots.add("dock", src, (46, 45), "unfocused", app)

    src = "tests/design_deep.py deep_swarm, SwarmApp (subagents to any depth)"
    app = SwarmApp(source=deep_swarm, live_refresh=False, clock=lambda: DNOW, focus_pane="w1:p3")
    async with app.run_test(size=(160, 45)) as pilot:
        await settle(app, pilot)
        for name, sel, focus in (("thread", None, 1), ("c1", "c1", 5)):
            app.view.thread, app.view.agent, app.view.focus = TID, sel, focus
            await frame(app, pilot, 0)
            shots.add("deep", src, (160, 45), name, app)

    src = "tests/test_machines.py mixed_swarm (a remote machine's group), SwarmApp"
    for size, file, cockpit_on in (((160, 45), "remote-wide", False), ((46, 45), "remote-dock", True)):
        def source(cockpit_on=cockpit_on):
            sw = mixed_swarm()
            sw.docked = "w1:p1" if cockpit_on else None
            return sw
        app = SwarmApp(source=source, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True,
                       cockpit_mode=cockpit_on)
        async with app.run_test(size=size) as pilot:
            await settle(app, pilot)
            await frame(app, pilot, 0)
            shots.add(file, src, size, "initial", app)


async def scenario_screens(shots: Shots, work: Path) -> None:
    """The transcript scenarios through the real collector (scanner + model), on screen."""
    from herdr_panorama.collect import Collector
    from herdr_panorama.inbox import open_questions
    from herdr_panorama.tui import SwarmApp
    from tests.scenario import SCENARIOS

    for name, build in SCENARIOS.items():
        base = work / "screens" / name
        base.mkdir(parents=True)
        d = build(base)
        file_times(d["projects"])
        os.environ["PANORAMA_EVENTS_DIR"] = str(d["events"])
        focus = d["snapshot"]["agents"][0]["pane_id"]
        src = f"tests/scenario.py {build.__name__} through Collector (now {d['now']}, procs off), SwarmApp"
        runs = [((160, 45), "wide", 0), ((70, 45), "narrow", 0), ((46, 45), "dock", 0)]
        if name == "demo":
            runs.append(((160, 45), "wide", 13 * 60))  # past the default stale_after (12h)
        for size, mode, later in runs:
            now = d["now"] + later * MIN
            collector = Collector(root=str(d["projects"]), now=lambda now=now: now, procs=False)

            def source(collector=collector, d=d, mode=mode):
                sw = collector.collect(d["snapshot"], open_questions(d["inbox"]))
                sw.docked = focus if mode == "dock" else None
                return sw
            cockpit_on = mode == "dock"
            app = SwarmApp(source=source, collector=collector, live_refresh=False, clock=lambda now=now: now,
                           focus_pane=focus, home_mode=cockpit_on, cockpit_mode=cockpit_on)
            async with app.run_test(size=size) as pilot:
                await settle(app, pilot)
                await frame(app, pilot, 0)
                shots.add(f"scenario-{name}-{mode}", src, size, f"+{later}m" if later else "initial", app)
                if mode == "wide" and not later:
                    await pilot.press("right")
                    await frame(app, pilot, 0)
                    shots.add(f"scenario-{name}-{mode}", src, size, "key 1 right", app)
        os.environ["PANORAMA_EVENTS_DIR"] = str(work / "no-events")


async def panel_screens(shots: Shots, work: Path) -> None:
    """A per-tab panel (panorama panel) drawing the core's published state."""
    from herdr_panorama import protocol
    from herdr_panorama.panel import PanelApp
    from tests.design_dock import FIX, design_swarm
    from tests.demo import NOW

    nine = json.loads((FIX / "design_9.json").read_text())["data"]
    src = "PanelApp (panorama panel) fed protocol.state_message of design_swarm (design_9.json data)"

    def state(visible):
        return protocol.state_message(design_swarm(nine), {}, None, visible, None)["state"]
    app = PanelApp(sock_path=work / "core.sock", pane_id="w1:p92", tab_id="w1:t2", clock=lambda: NOW,
                   start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app._got_state(state(["w1:t2"]))
        await pilot.pause()
        await frame(app, pilot, 0)
        shots.add("panel", src, (46, 45), "tab 2 shown", app)
        app._got_state(state(["w1:t4"]))
        await pilot.pause()
        await frame(app, pilot, 0)
        shots.add("panel", src, (46, 45), "tab 4 shown", app)


def capture_screens(out: Path, work: Path) -> list[Path]:
    from tests.conftest import FakeHerdr
    fake = FakeHerdr(work)
    os.environ.update(HERDR_BIN_PATH=str(fake.bin), FAKE_HERDR_LOG=str(fake.log))
    from herdr_panorama import cockpit
    cockpit.moving_until = 0.0
    shots = Shots()

    async def run():
        await swarm_screens(shots)
        await scenario_screens(shots, work)
        await panel_screens(shots, work)
    asyncio.run(run())
    written = []
    for name, doc in shots.files.items():
        doc["source"] += "; TZ=UTC, frame 0 unless the screen's name says; tools/capture_goldens.py"
        path = out / "screens" / f"{name}.json"
        write(path, doc)
        written.append(path)
    return written


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--out", type=Path, default=FIXTURES, help="write OUT/screens and OUT/model (default: %(default)s)")
    p.add_argument("--only", choices=("screens", "model"), help="one of the two sets")
    args = p.parse_args(argv)
    work = Path(tempfile.mkdtemp(prefix="panorama-goldens-"))
    try:
        isolate(work)
        for kind in ("screens", "model"):
            if args.only in (None, kind):
                shutil.rmtree(args.out / kind, ignore_errors=True)
        written = []
        if args.only in (None, "model"):
            written += capture_model(args.out, work)
        if args.only in (None, "screens"):
            written += capture_screens(args.out, work)
        size = sum(f.stat().st_size for f in written)
        print(f"{len(written)} files, {size // 1024} KiB under {args.out}")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
