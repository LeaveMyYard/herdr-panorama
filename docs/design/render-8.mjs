// Render turn 8 ("waiting and stale") from the reference JS.
// Usage: node render-8.mjs json > 8.json  (frames 0, 4, 7, 12 and 1.5, 3.4 between frames: A the 2a graph with Deploy Watch's background card,
//                                          B the 2a graph with the waiting agent n1, R the same with the "running"
//                                          filter, P the 46 docked-panel columns; chars and colours per cell)
//        node render-8.mjs html > 8.html  (a static page for a screenshot)
import { renderStates } from './swarm-states.js';
const mode = process.argv[2] || 'html';
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
if (mode === 'json') {
  const frames = {};
  for (const f of [0, 4, 7, 12, 1.5, 3.4]) { const r = renderStates(f); frames[f] = { A: cells(r.A), B: cells(r.B), R: cells(r.R), P: cells(r.P).map(x => x.slice(0, 46)) }; }
  console.log(JSON.stringify(frames));
} else {
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  const grid = (rows, w) => `<div class="g" style="width:${w}ch">` + rows.map(x => '<div class="r">' + x.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('') + '</div>';
  const a = renderStates(0);
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#e6e4df;display:flex;flex-wrap:wrap;gap:16px;padding:16px;width:2700px}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px;overflow:hidden}.r{display:flex;height:16px;white-space:pre}</style>${grid(a.A, 160)}${grid(a.P, 120)}${grid(a.B, 160)}${grid(a.R, 160)}`);
}
