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
    """The cockpit's docked panel (3a) next to the design's own render of the same dataset and
    frame: docs/screenshots/dock-3a-app.png and dock-3a-design.png."""
    from .design_dock import design_swarm
    app = SwarmApp(source=design_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p3", home_mode=True,
                   cockpit_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        app.view.frame, app.view.hover = 4, ("orch", ("w1:p4",))  # the design's frame and hover
        app.repaint()
        await pilot.pause()
        shot(app, "dock-3a-app", 600)
    html = OUT / "dock-3a-design.html"
    design = Path(__file__).resolve().parent.parent / "docs" / "design"
    html.write_text(subprocess.run(["node", "render-3a.mjs", "4", "html"], cwd=design, capture_output=True,
                                   text=True, check=True).stdout)
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--window-size=384,752", f"--screenshot={OUT / 'dock-3a-design.png'}", html.as_uri()],
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
