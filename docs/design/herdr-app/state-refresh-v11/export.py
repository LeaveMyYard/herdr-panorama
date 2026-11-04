#!/usr/bin/env python3
"""Export the offline synthetic V11 DESIGN proposal to PNGs; never opens herdr-app.

Stages a copy of this folder (plus ../icons, ../ui and, if given, Inter font files) in a
scratch directory so nothing font-related is committed, renders each frame with headless
Chrome, then writes the PNGs and comparison sheets back here.
"""
import argparse, json, re, shutil, subprocess, tempfile
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont, PngImagePlugin

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('--chrome', default=shutil.which('google-chrome-stable') or '/opt/google/chrome/chrome')
ap.add_argument('--scratch', type=Path, required=True, help='existing private scratch directory')
ap.add_argument('--fonts', type=Path, help='directory holding Inter-Regular/Medium/SemiBold.ttf (v2 was drawn in Inter)')
ap.add_argument('--frames', nargs='+', default=['A', 'B', 'C', 'D', 'D2', 'Gallery'])
args = ap.parse_args()
here = Path(__file__).resolve().parent
FRAMES = {'A': (1920, 1080, 'v11-a-window.png'), 'B': (1208, 2360, 'v11-b-sidebar.png'), 'C': (1288, 1800, 'v11-c-peek.png'),
          'D': (1920, 1080, 'v11-d-graph-tab.png'), 'D2': (1920, 1080, 'v11-d2-graph-scrolled.png'), 'Gallery': (1600, 1260, 'state-gallery.png')}

stage = Path(tempfile.mkdtemp(prefix='panorama-v11-', dir=args.scratch))
shutil.copytree(here, stage / 'state-refresh-v11', ignore=shutil.ignore_patterns('*.png'))
for d in ('icons', 'ui'):
    shutil.copytree(here.parent / d, stage / d)
if args.fonts:
    (stage / 'state-refresh-v11' / 'fonts').mkdir()
    for f in ('Inter-Regular.ttf', 'Inter-Medium.ttf', 'Inter-SemiBold.ttf'):
        shutil.copy(args.fonts / f, stage / 'state-refresh-v11' / 'fonts' / f)
page = stage / 'state-refresh-v11' / 'preview.html'

reports = {}
for key in args.frames:
    w, h, name = FRAMES[key]
    with tempfile.TemporaryDirectory(prefix='profile-', dir=stage) as prof:
        out = stage / name
        cmd = [args.chrome, '--headless', '--no-sandbox', '--disable-gpu', '--hide-scrollbars', '--allow-file-access-from-files',
               '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-extensions', '--disable-sync',
               '--force-device-scale-factor=1', '--force-color-profile=srgb', '--virtual-time-budget=1500', '--user-data-dir=' + prof,
               f'--window-size={w},{h}', '--screenshot=' + str(out), '--dump-dom', page.as_uri() + f'?frame={key}&export=1']
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=60, check=True)
    m = re.search(r'<script id="design-report" type="application/json">(.*?)</script>', res.stdout, re.S)
    if not m:
        raise RuntimeError('missing render report: ' + key)
    rep = json.loads(m.group(1)); reports[key] = rep
    img = Image.open(out); assert img.size == (w, h), (name, img.size)
    meta = PngImagePlugin.PngInfo(); meta.add_text('Description', 'Synthetic Panorama V11 design proposal. Not an installed/live screenshot.')
    img.save(here / name, pnginfo=meta)
    print(key, name, 'inter' if rep.get('interLoaded') else 'FALLBACK FONT', 'tiles', rep.get('badgeCount'), 'overflow', rep.get('badgeTextOverflow'), rep.get('rowOverflow'))
(here / 'render-checks.json').write_text(json.dumps(reports, indent=2, ensure_ascii=False) + '\n')

# ---- comparison sheets (presentation only; source PNGs untouched) ----
FONT = next((p for p in ['/usr/share/fonts/TTF/DejaVuSans.ttf', '/usr/share/fonts/noto/NotoSans-Regular.ttf'] if Path(p).exists()), None)
font = ImageFont.truetype(FONT, 22) if FONT else ImageFont.load_default()
v2, v3 = here.parent, here.parent / 'state-refresh'

def sheet(items, name, crop=None, scale=1, cols=None):
    ims = []
    for label, path in items:
        im = Image.open(path).convert('RGB')
        if crop: im = im.crop(crop)
        if scale != 1: im = im.resize((round(im.width * scale), round(im.height * scale)), Image.LANCZOS)
        ims.append((label, im))
    cols = cols or len(ims); rows = -(-len(ims) // cols)
    cw, ch = ims[0][1].size; gap, top = 24, 52
    canvas = Image.new('RGB', (cols * cw + (cols + 1) * gap, rows * (ch + top) + (rows + 1) * gap - top + top), '#e9e7e1')
    d = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(ims):
        x = gap + (i % cols) * (cw + gap); y = gap + (i // cols) * (ch + top + gap)
        d.text((x, y + 12), label, fill='#252a32', font=font); canvas.paste(im, (x, y + top))
    canvas.save(here / name)

if all((here / f).exists() for f in ('v11-a-window.png', 'v11-d-graph-tab.png', 'v11-d2-graph-scrolled.png')):
    # sidebar: the real TUI dock capture next to V5 and V11 (each scaled to the same height)
    tui = Image.open(here.parents[2] / 'screenshots' / 'dock-app.png').convert('RGB').crop((0, 40, 600, 620))
    v5 = Image.open(here.parent / 'state-refresh-v10' / 'v10-a-window.png').convert('RGB').crop((0, 230, 384, 1000))
    v6 = Image.open(here / 'v11-a-window.png').convert('RGB').crop((0, 230, 384, 1000))
    H = 1540; ims = [(l, im.resize((round(im.width * H / im.height), H), Image.LANCZOS)) for l, im in
                     [('TUI dock today (docs/screenshots/dock-app.png)', tui), ('V10', v5), ('V11 proposal', v6)]]
    gap, top = 24, 52; W = sum(im.width for _, im in ims) + gap * (len(ims) + 1)
    canvas = Image.new('RGB', (W, H + top + 2 * gap), '#e9e7e1'); d = ImageDraw.Draw(canvas); x = gap
    for l, im in ims:
        d.text((x, gap + 12), l, fill='#252a32', font=font); canvas.paste(im, (x, gap + top)); x += im.width + gap
    canvas.save(here / 'compare-sidebar.png')
    sheet([('Approved v2', v2 / 'v2-d-graph-tab.png'), ('V10', here.parent / 'state-refresh-v10' / 'v10-d-graph-tab.png'), ('V11 proposal', here / 'v11-d-graph-tab.png')],
          'compare-graph.png', crop=(396, 100, 1910, 560), cols=1)
    sheet([('Approved v2', v2 / 'v2-d2-graph-scrolled.png'), ('V10', here.parent / 'state-refresh-v10' / 'v10-d2-graph-scrolled.png'), ('V11 proposal', here / 'v11-d2-graph-scrolled.png')],
          'compare-graph-deep.png', crop=(396, 100, 1910, 680), cols=1)
shutil.rmtree(stage)
