"""Regenerate docs/screenshots/*.png from the neutral demo swarm (tests/demo.py).

Not collected by a plain `pytest` run. Usage (needs Chrome or Chromium; rsvg-convert squeezes the
leading no-break spaces of Textual's SVG export):

    uv run pytest tests/make_screenshots.py -q
"""

import re
import shutil
import subprocess
from pathlib import Path

from herdr_panorama.tui import SwarmApp

from .demo import NOW, demo_swarm

OUT = Path(__file__).resolve().parent.parent / "docs" / "screenshots"


CHROME = next((c for c in ("google-chrome-stable", "google-chrome", "chromium", "chromium-browser") if shutil.which(c)), None)


def shot(app, name: str, width_px: int) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    svg_text = app.export_screenshot(title="herdr panorama")
    m = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg_text)
    w, h = (float(m.group(1)), float(m.group(2))) if m else (2000.0, 1200.0)
    scale = width_px / w
    html = OUT / f"{name}.html"
    html.write_text(f"<!doctype html><meta charset=utf-8><style>body{{margin:0;background:#101215}}"
                    f"svg{{width:{width_px}px;height:{int(h * scale)}px;display:block}}</style>{svg_text}")
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    f"--window-size={width_px},{int(h * scale)}", f"--screenshot={OUT / (name + '.png')}",
                    html.as_uri()], check=True, capture_output=True, timeout=60)
    html.unlink()


async def test_shots(fake_herdr):
    for name, size, focus, keys, px in (
        ("wide-160x45", (160, 45), "w1:p1", ["right", "right", "down"], 2000),
        ("narrow-70x45", (70, 45), "w1:p1", ["right", "down", "down", "right"], 900),
    ):
        app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane=focus)
        async with app.run_test(size=size) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            for k in keys:
                await pilot.press(k)
            app.view.frame = 0
            app.repaint()
            await pilot.pause()
            shot(app, name, px)


async def test_new_agent_shot(fake_herdr):
    """The home tab with the + New agent form open, typed in."""
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        await pilot.press("n")
        await pilot.pause()
        app.screen.query_one("#cwd").value = "/home/user/work/billing"
        await pilot.press(*"Billing export")
        await pilot.press("tab", "tab")
        await pilot.press(*"Add CSV export to the invoices page")
        await pilot.pause()
        shot(app, "new-agent", 2000)


async def test_dock_panel_shots(fake_herdr):
    """The cockpit's docked panel (9e, turn 10's default fixed colours) focused and not, next to
    the design's own render of the same dataset and frame: docs/screenshots/dock-app.png,
    dock-app-unfocused.png, dock-design.png and dock-design-unfocused.png."""
    import json

    from textual import events

    from .design_dock import FIX, design_swarm
    data = json.loads((FIX / "design_9.json").read_text())["data"]
    app = SwarmApp(source=lambda: design_swarm(data), live_refresh=False, clock=lambda: NOW, focus_pane="w1:p3",
                   home_mode=True, cockpit_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        app.view.frame, app.view.hover = 4, ("orch", ("w1:p4",))  # the design's frame and hover
        app.repaint()
        await pilot.pause()
        shot(app, "dock-app", 600)
        app.post_message(events.AppBlur())
        await pilot.pause()
        shot(app, "dock-app-unfocused", 600)
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    for key, name in (("yoursA", "dock-design"), ("yoursI", "dock-design-unfocused")):
        html = OUT / f"{name}.html"
        html.write_text(subprocess.run(["node", "render-10.mjs", "html", key, "4"], cwd=design, capture_output=True,
                                       text=True, check=True).stdout)
        subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                        "--window-size=384,752", f"--screenshot={OUT / (name + '.png')}", html.as_uri()],
                       check=True, capture_output=True, timeout=60)
        html.unlink()


async def test_remote_group_shots(fake_herdr):
    """A remote machine's orchestrators under their divider: docs/screenshots/remote-*.png."""
    from .test_machines import mixed_swarm
    for name, size, px, cockpit_on in (("remote-wide", (160, 45), 2000, False), ("remote-dock", (46, 45), 600, True)):
        def source():
            sw = mixed_swarm()
            sw.docked = "w1:p1" if cockpit_on else None
            return sw
        app = SwarmApp(source=source, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True,
                       cockpit_mode=cockpit_on)
        async with app.run_test(size=size) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            app.view.frame = 0
            app.repaint()
            await pilot.pause()
            shot(app, name, px)


async def test_deep_shots(fake_herdr):
    """Subagents to any depth (design turn 5, 5d) on the design's own dataset, next to the
    design's render: docs/screenshots/deep-5d-{thread,b1,c1,dock}-{app,design}.png."""
    from .design_deep import NOW as DNOW, TID, deep_swarm
    for name, sel, focus in (("thread", None, 1), ("b1", "b1", 3), ("c1", "c1", 5)):
        app = SwarmApp(source=deep_swarm, live_refresh=False, clock=lambda: DNOW, focus_pane="w1:p3")
        async with app.run_test(size=(160, 45)) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            app.view.thread, app.view.agent, app.view.focus, app.view.frame = TID, sel, focus, 4
            app.repaint()
            await pilot.pause()
            shot(app, f"deep-5d-{name}-app", 2000)

    def source():
        sw = deep_swarm()
        sw.docked = "w1:p3"
        return sw
    app = SwarmApp(source=source, live_refresh=False, clock=lambda: DNOW, focus_pane="w1:p3", home_mode=True,
                   cockpit_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        app.view.frame = 4
        app.repaint()
        await pilot.pause()
        shot(app, "deep-5d-dock-app", 600)
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    html = OUT / "deep-5d-design.html"
    html.write_text(subprocess.run(["node", "render-5d.mjs", "4", "html"], cwd=design, capture_output=True,
                                   text=True, check=True).stdout)
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--window-size=2700,1540", f"--screenshot={OUT / 'deep-5d-design.png'}", html.as_uri()],
                   check=True, capture_output=True, timeout=60)
    html.unlink()


async def test_unseen_shots(fake_herdr):
    """Unseen (design turn 6) on the design's dataset at the pulse's peak (frame 6), next to the
    design's render: docs/screenshots/unseen-{2a,3a}-app.png and unseen-design.png."""
    from .design_dock import design_swarm
    marks = {"b3": "fail", "c1": "wait", "a5": "done", "i3": "wait", "k2": "done"}
    for name, size, px, cockpit_on in (("unseen-2a-app", (160, 45), 2000, False), ("unseen-3a-app", (46, 45), 600, True)):
        def source():
            sw = design_swarm()
            sw.docked = "w1:p3" if cockpit_on else None
            return sw
        app = SwarmApp(source=source, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1" if not cockpit_on else "w1:p3",
                       home_mode=True, cockpit_mode=cockpit_on)
        async with app.run_test(size=size) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            v = app.view
            if cockpit_on:
                v.orch, v.hover = "w1:p3", ("orch", ("w1:p4",))
            else:
                v.orch, v.thread, v.agent, v.focus = "w1:p1", "t595", "a2", 2
            v.unseen, v.frame = dict(marks), 6
            app.repaint()
            await pilot.pause()
            shot(app, name, px)
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    html = OUT / "unseen-design.html"
    html.write_text(subprocess.run(["node", "render-6.mjs", "html"], cwd=design, capture_output=True,
                                   text=True, check=True).stdout)
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--window-size=2700,1560", f"--screenshot={OUT / 'unseen-design.png'}", html.as_uri()],
                   check=True, capture_output=True, timeout=60)
    html.unlink()


async def test_wide_shots(fake_herdr):
    """Wider cards (design turn 7) on its dataset at 240 and 320 columns, next to the design:
    docs/screenshots/wide-{240,320}-app.png and wide-design.png."""
    from .design_wide import NOW as WNOW, TID, wide_swarm
    t = wide_swarm().orchestrators[0].threads[0]
    r1 = next(a.id for a in t.agents if a.depth == 2 and a.activity.startswith("tracing"))
    for w, px in ((240, 2400), (320, 3000)):
        app = SwarmApp(source=wide_swarm, live_refresh=False, clock=lambda: WNOW, focus_pane="w1:p3")
        async with app.run_test(size=(w, 45)) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            app.view.thread, app.view.agent, app.view.focus, app.view.frame = TID, r1, 3, 4
            app.repaint()
            await pilot.pause()
            shot(app, f"wide-{w}-app", px)
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    page = subprocess.run(["node", "-e", """
import('./swarm-wide.js').then(m => {
  const r = m.renderWide(4);
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, c => '-' + c.toLowerCase()) + ':' + v).join(';');
  const grid = (rows, w) => `<div class="g" style="width:${w}ch">` + rows.map(x => '<div class="r">' + x.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('') + '</div>';
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#e6e4df;display:flex;flex-direction:column;gap:16px;padding:16px}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px}.r{display:flex;height:16px;white-space:pre}</style>${grid(r.W240, 240)}${grid(r.R320, 320)}`);
});"""], cwd=design, capture_output=True, text=True, check=True).stdout
    html = OUT / "wide-design.html"
    html.write_text(page)
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--window-size=2400,1580", f"--screenshot={OUT / 'wide-design.png'}", html.as_uri()],
                   check=True, capture_output=True, timeout=60)
    html.unlink()


async def test_states_shots(fake_herdr):
    """Waiting and stale (design turn 8) on the design's dataset, next to the design's render:
    docs/screenshots/states-{2a,agent,3a}-app.png and states-design.png."""
    from .test_states import NOW as SNOW
    from .test_states import states_swarm
    for name, size, px, cockpit_on, sel in (
            ("states-2a-app", (160, 56), 2000, False, ("w1:p5", "bg:w1:p5", None)),
            ("states-agent-app", (160, 56), 2000, False, ("w1:p4", "p214", "n1")),
            ("states-3a-app", (46, 56), 600, True, ("w1:p5", None, None))):
        def source():
            sw = states_swarm()
            sw.generated_at = SNOW
            if not cockpit_on:
                sw.docked = None
            return sw
        app = SwarmApp(source=source, live_refresh=False, clock=lambda: SNOW, focus_pane=sel[0],
                       home_mode=True, cockpit_mode=cockpit_on)
        async with app.run_test(size=size) as pilot:
            for _ in range(50):
                await pilot.pause(0.02)
                if app.loaded:
                    break
            v = app.view
            v.orch, v.thread, v.agent, v.focus = sel[0], sel[1], sel[2], 0 if cockpit_on else 2
            if cockpit_on:
                v.hover = ("orch", ("w1:p7",))
            app.repaint()
            await pilot.pause()
            shot(app, name, px)
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    html = OUT / "states-design.html"
    html.write_text(subprocess.run(["node", "render-8.mjs", "html"], cwd=design, capture_output=True,
                                   text=True, check=True).stdout)
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--window-size=2700,1920", f"--screenshot={OUT / 'states-design.png'}", html.as_uri()],
                   check=True, capture_output=True, timeout=60)
    html.unlink()
