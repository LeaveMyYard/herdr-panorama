// Render turn 6 ("unseen") from the reference JS.
// Usage: node render-6.mjs json > 6.json  (frames 0, 3, 6, 9: G the 160x45 graph, N the same with nothing unseen,
//                                          P the 46 panel columns; chars and colours per cell)
//        node render-6.mjs html > 6.html  (a static page for a screenshot)
import { renderUnseen } from './swarm-unseen.js';
const mode = process.argv[2] || 'html';
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
if (mode === 'json') {
  const frames = {};
  for (const f of [0, 3, 6, 9]) { const n = renderUnseen(f, {}).G, r = renderUnseen(f); frames[f] = { G: cells(r.G), N: cells(n), P: cells(r.P).map(x => x.slice(0, 46)) }; }
  console.log(JSON.stringify(frames));
} else {
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  const grid = (rows, w) => `<div class="g" style="width:${w}ch">` + rows.map(x => '<div class="r">' + x.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('') + '</div>';
  const a = renderUnseen(0), b = renderUnseen(6);
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#e6e4df;display:flex;flex-wrap:wrap;gap:16px;padding:16px;width:2700px}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px;overflow:hidden}.r{display:flex;height:16px;white-space:pre}</style>${grid(a.G, 160)}${grid(b.G, 160)}${grid(a.P, 46)}${grid(b.P, 46)}`);
}
