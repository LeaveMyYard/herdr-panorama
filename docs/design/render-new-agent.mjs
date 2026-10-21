// Render the new-agent modal, design 1C "Coming soon" (`modalCSoon`), from the reference JS.
// Usage: node render-new-agent.mjs > new_agent.json
//   frames: modalCSoon(sel) for each harness, 96x32 (the modal 76x27 at 10, 3), with its click targets;
//   input: the design's own `input()` focused with its cursor (not exported: it is reached by
//   re-exporting it from the unchanged module source), on a 76x6 panel at x 3: Name with its sub line and
//   Working directory. Chars and colours per cell: [ch, fg, bg|null, bold].
import { readFileSync } from 'node:fs';
const src = readFileSync(new URL('./herdr-tui.js', import.meta.url), 'utf8');
const mod = await import('data:text/javascript;base64,' + Buffer.from(src + '\nexport { input, Scr, nameSub };\n').toString('base64'));
const { modalCSoon, HARNESS, C, input, Scr, nameSub } = mod;
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
const frames = HARNESS.map((h, sel) => { const o = modalCSoon(sel, 0); return { sel, id: h.id, rows: cells(o.rows), hits: o.hits }; });
const field = (label, value, sub) => { const s = new Scr(76, 6, C.panel); input(s, 3, 0, 70, label, value, { focus: true, cursor: true, sub }); return cells(s.out().rows); };
const input_ = { name: field('Name (also the tab label)', 'fix tab prefix stacking', nameSub), cwd: field('Working directory', '~/Code/herdr') };
console.log(JSON.stringify({ harness: HARNESS.map(h => ({ id: h.id, name: h.name, cmd: h.cmd, col: h.col, big: h.big })), C, frames, input: input_ }));
