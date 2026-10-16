"""Presentation frame for README images: a soft gradient backdrop, padding and a drop shadow.

The same look as herdr-inbox's README images (its docs/screenshots/frame.py). The window
itself (rounded corners, title bar with the three dots) comes from the capture: Textual's
SVG export draws it, and this frame keeps its transparent corners.

    uv run --with pillow python docs/screenshots/frame.py in.png out.png
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageFilter

TOP, BOTTOM = (31, 37, 71), (66, 45, 99)  # slate indigo -> muted violet, diagonal
PAD = 80
SHADOW_BLUR, SHADOW_OFFSET, SHADOW_ALPHA = 28, 18, 150


def gradient(size: tuple[int, int]) -> Image.Image:
    w, h = size
    small = Image.new("RGB", (64, 64))
    px = small.load()
    for y in range(64):
        for x in range(64):
            t = x / 63 * 0.45 + y / 63 * 0.55
            px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(TOP, BOTTOM))
    return small.resize((w, h), Image.BICUBIC)


def frame(img: Image.Image, pad: int = PAD) -> Image.Image:
    """``img`` (RGBA, transparent outside the window) on the gradient, with a soft shadow."""
    img = img.convert("RGBA")
    w, h = img.size
    canvas = gradient((w + 2 * pad, h + 2 * pad)).convert("RGBA")
    shadow = Image.new("L", canvas.size, 0)
    shadow.paste(img.getchannel("A").point(lambda a: SHADOW_ALPHA if a else 0), (pad, pad + SHADOW_OFFSET))
    shadow = shadow.filter(ImageFilter.GaussianBlur(SHADOW_BLUR))
    canvas.paste(Image.new("RGBA", canvas.size, (8, 8, 20, 255)), (0, 0), shadow)
    canvas.alpha_composite(img, (pad, pad))
    return canvas.convert("RGB")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("src", type=Path)
    ap.add_argument("dst", type=Path)
    ap.add_argument("--pad", type=int, default=PAD)
    args = ap.parse_args()
    frame(Image.open(args.src), args.pad).save(args.dst, optimize=True)


if __name__ == "__main__":
    main()
