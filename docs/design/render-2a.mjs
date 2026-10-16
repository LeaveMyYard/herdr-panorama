// Render variant 2a (both layouts) from the reference JS to a static HTML page.
// Usage: node render-2a.mjs > 2a.html ; google-chrome-stable --headless --screenshot=2a.png 2a.html
import { renderMin } from './swarm-min.js';  // frame 4: the working bar is mid-card
const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
const grid = (rows, w) => `<div class="g" style="width:${w}ch">` + rows.map(r => '<div class="r">' + r.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('') + '</div>';
const m = renderMin(4);
console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#e6e4df;display:flex;gap:28px;padding:20px;align-items:flex-start}
.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px}.r{display:flex;height:16px;white-space:pre}</style>
${grid(m.G1, 160)}${grid(m.G2, 70)}`);
