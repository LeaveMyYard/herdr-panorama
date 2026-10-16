// Render turn 5 ("subagents to any depth"), direction 5d, from the reference JS.
// Usage: node render-5d.mjs [frame] json > 5d.json  (V4..V6: the 160x45 graph frames, P4: the 46-column panel;
//                                                     chars and colours per cell)
//        node render-5d.mjs [frame] html > 5d.html  (a static page for a screenshot)
import { renderDeep } from './swarm-deep.js';
const f = Number(process.argv[2] || 4), mode = process.argv[3] || 'html';
const r = renderDeep(f), K = ['V4', 'V5', 'V6', 'P4'];
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
if (mode === 'json') {
  console.log(JSON.stringify({ frame: f, ...Object.fromEntries(K.map(k => [k, cells(r[k])])) }));
} else {
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  const grid = (rows, w) => `<div class="g" style="width:${w}ch">` + rows.map(x => '<div class="r">' + x.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('') + '</div>';
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#e6e4df;display:flex;flex-wrap:wrap;gap:16px;padding:16px;width:2700px}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px}.r{display:flex;height:16px;white-space:pre}</style>${grid(r.V4, 160)}${grid(r.P4, 46)}${grid(r.V5, 160)}${grid(r.V6, 160)}`);
}
