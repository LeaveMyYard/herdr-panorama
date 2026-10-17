"""Record docs/screenshots/hero.gif: the real TUI (wide 160x45), neutral demo data, animated.

Each frame is the actual renderer's output: the Textual app runs headless on the demo swarm
(tests/demo.py), steps one animation frame at a time (the 250 ms design timing), and every
frame is exported as SVG, rendered to PNG by headless Chrome (transparent around the
window), framed like the herdr-inbox README images (frame.py), and assembled into a
palette-optimised GIF by ffmpeg. One cursor move down and back keeps the loop seamless.

    uv run --with pillow python docs/screenshots/hero_gif.py      # needs Chrome/Chromium + ffmpeg
"""

from __future__ import annotations

import asyncio
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT), str(ROOT / "src"), str(Path(__file__).parent)]

from frame import frame  # noqa: E402
from PIL import Image  # noqa: E402

from herdr_panorama.tui import SwarmApp  # noqa: E402
from tests.anim import at_frame  # noqa: E402
from tests.demo import NOW, demo_swarm  # noqa: E402

OUT = Path(__file__).resolve().parent / "hero.gif"
FRAMES = 32  # one full sweep of a card's working bar (28-wide card: 26 + 6 positions)
MOVES = {10: "down", 22: "up"}  # one cursor move in the thread column, then back
WIDTH = 1380  # pixels; shown at 920 in the README
CHROME = next((c for c in ("google-chrome-stable", "google-chrome", "chromium", "chromium-browser") if shutil.which(c)),
              None)


async def capture(tmp: Path) -> list[Path]:
    os.environ.setdefault("HERDR_BIN_PATH", str(tmp / "no-herdr"))  # the demo never calls herdr
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1")
    svgs = []
    async with app.run_test(size=(160, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        await pilot.press("right")  # threads column: PR #42 with its steps and agent cards
        for f in range(FRAMES):
            if f in MOVES:
                await pilot.press(MOVES[f])
            at_frame(app, f)  # the frame follows the anim clock: hold it at f
            app.repaint()
            await pilot.pause()
            svg = tmp / f"f{f:03d}.svg"
            svg.write_text(app.export_screenshot(title="herdr panorama"))
            svgs.append(svg)
    return svgs


def to_png(svg: Path) -> Path:
    text = svg.read_text()
    m = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', text)
    w, h = float(m.group(1)), float(m.group(2))
    html = svg.with_suffix(".html")
    html.write_text(f"<!doctype html><meta charset=utf-8><style>html,body{{margin:0;background:transparent}}"
                    f"svg{{width:{w:.0f}px;height:{h:.0f}px;display:block}}</style>{text}")
    png = svg.with_suffix(".png")
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--default-background-color=00000000", f"--window-size={w:.0f},{h:.0f}",
                    f"--screenshot={png}", html.as_uri()], check=True, capture_output=True, timeout=60)
    return png


def main() -> None:
    if CHROME is None or shutil.which("ffmpeg") is None:
        raise SystemExit("needs Chrome or Chromium and ffmpeg")
    with tempfile.TemporaryDirectory(dir=ROOT / ".scratch" if (ROOT / ".scratch").is_dir() else None) as d:
        tmp = Path(d)
        svgs = asyncio.run(capture(tmp))
        for i, svg in enumerate(svgs):
            img = frame(Image.open(to_png(svg)))
            img = img.resize((WIDTH, round(img.height * WIDTH / img.width)), Image.LANCZOS)
            img.save(tmp / f"p{i:03d}.png")
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "4", "-i", str(tmp / "p%03d.png"),
                        "-vf", "split[a][b];[a]palettegen=max_colors=128:stats_mode=full[p];"
                               "[b][p]paletteuse=dither=none:diff_mode=rectangle",
                        "-loop", "0", str(OUT)], check=True)
    print(f"{OUT} {OUT.stat().st_size / 1e6:.2f} MB")


if __name__ == "__main__":
    main()
