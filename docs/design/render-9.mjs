// Render turn 9's docked panel, variant 9e (the operator's pick), from the reference JS.
// Usage: node render-9.mjs json > 9.json   per frame: E (9e), U (9e with the unseen sample marks);
//        the 46 panel columns as [char, fg, bg, bold] cells, plus the dataset (swarm-tui.js + Docs Site)
//        node render-9.mjs html [frame] > 9e.html   9e at that frame, a static page for a screenshot
import { renderTiles } from './swarm-tiles.js';
import { DATA } from './swarm-tui.js';
const cells = rows => rows.map(r => { const out = []; for (const u of r.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out.slice(0, 46); });
if (process.argv[2] === 'html') {
  const rows = renderTiles(Number(process.argv[3] || 4)).PE;
  const esc = t => t.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  const grid = rows.map(r => '<div class="r">' + r.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('');
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#101215}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px;width:46ch;overflow:hidden}.r{display:flex;height:16px;white-space:pre}</style><div class="g">${grid}</div>`);
  process.exit(0);
}
const frames = {};
for (const f of [0, 1, 2, 3, 4, 6, 7, 9]) { const r = renderTiles(f); frames[f] = { E: cells(r.PE), U: cells(r.PU) }; }
const data = DATA.map(o => ({ name: o.name, tab: o.tab, fl: o.fl, threads: o.threads.map(t => ({
  id: t.id, name: t.name, ph: t.ph.map(p => [p[0], p[1]]),
  agents: t.agents.map(a => ({ id: a.id, role: a.role, model: a.model || '', st: a.st, q: a.q || 0, n: a._n,
    act: a.act || '', msg: a.msg || '', qs: a.qs || [], err: a.err || null, perm: a.perm || null })) })) }));
console.log(JSON.stringify({ data, frames }));
