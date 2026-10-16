// Render turn 7 ("wider cards") from the reference JS.
// Usage: node render-7.mjs [frame] json > 7.json  (W160/W240/W320 focus on the thread, R160/R320 focus on depth 2,
//                                                   S the sweep strip; chars and colours per cell)
import { renderWide } from './swarm-wide.js';
const f = Number(process.argv[2] || 4);
const r = renderWide(f);
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
console.log(JSON.stringify({ frame: f, ...Object.fromEntries(Object.entries(r).map(([k, v]) => [k, cells(v)])) }));
