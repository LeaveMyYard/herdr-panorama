// Render variant 3a "Compact list" (the docked cockpit panel) from the reference JS.
// Usage: node render-3a.mjs [frame] html > 3a.html  (a static page for a screenshot)
//        node render-3a.mjs [frame] json > 3a.json  (the 46 panel columns: chars and colours per cell)
import { renderDock } from './swarm-dock.js';
const f = Number(process.argv[2] || 4), mode = process.argv[3] || 'html';
const rows = renderDock(f).K1;
if (mode === 'json') {
  const out = rows.map(r => { const cells = []; for (const u of r.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) cells.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return cells.slice(0, 46); });
  console.log(JSON.stringify({ frame: f, rows: out }));
} else {
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  // the 46 panel columns only (the design draws the chat stub to their right)
  const cut = r => { const o = []; let n = 0; for (const c of r.r) { const w = parseInt(c.st.width); if (n >= 46) break; if (n + w > 46) { const k = 46 - n; o.push({ ...c, t: Array.from(c.t).slice(0, k).join(''), st: { ...c.st, width: k + 'ch' } }); n = 46; break; } o.push(c); n += w; } return o; };
  const grid = rows.map(r => '<div class="r">' + cut(r).map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('');
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#101215}.g{background:#101215;padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px;width:46ch;overflow:hidden}.r{display:flex;height:16px;white-space:pre}</style><div class="g">${grid}</div>`);
}
