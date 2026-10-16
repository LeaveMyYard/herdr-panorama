// Render turn 10 (the docked 9e panel focused / not focused in herdr's theme) from the reference JS.
// Usage: node render-10.mjs json > 10.json   per frame: yoursA/yoursI (catppuccin + active_row_bg #414868,
//        focused / not), tokyoA/tokyoI (tokyo-night); the 46 panel columns as [char, fg, bg, bold] cells
//        node render-10.mjs html <key> [frame] > 10.html   one of them, a static page for a screenshot
import { renderFocus, SHOWN } from './swarm-focus.js';
const cells = rows => rows.map(r => { const out = []; for (const u of r.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out.slice(0, 46); });
if (process.argv[2] === 'html') {
  const key = process.argv[3] || 'yoursA', rows = renderFocus(Number(process.argv[4] || 4))[key];
  const bg = rows[1].r[0].st.background;
  const esc = t => t.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const css = o => Object.entries(o).filter(([, v]) => v !== undefined).map(([k, v]) => k.replace(/[A-Z]/g, m => '-' + m.toLowerCase()) + ':' + v).join(';');
  const grid = rows.map(r => '<div class="r">' + r.r.map(c => `<span style="${css(c.st)}">${esc(c.t)}</span>`).join('') + '</div>').join('');
  console.log(`<!doctype html><meta charset="utf-8"><style>body{margin:0;background:${bg}}.g{background:${bg};padding:14px 16px;font-family:'JetBrains Mono','DejaVu Sans Mono',monospace;font-size:12px;line-height:16px;width:46ch;overflow:hidden}.r{display:flex;height:16px;white-space:pre}</style><div class="g">${grid}</div>`);
  process.exit(0);
}
const frames = {};
for (const f of [0, 1, 4, 6]) { const r = renderFocus(f); frames[f] = Object.fromEntries(Object.entries(r).map(([k, v]) => [k, cells(v)])); }
console.log(JSON.stringify({ themes: Object.fromEntries(SHOWN.map(s => [s.key, s.t])), frames }));
