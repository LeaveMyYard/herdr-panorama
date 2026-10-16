// Render turn 8 (smooth animation) from the reference JS: turn 6's unseen screens between whole frames,
// where the pulse and the attention blink are mid-way (swarm-smooth.js).
// Usage: node render-8.mjs json > 8.json  (G the 160x45 graph, N the same with nothing unseen, P the 46 panel columns)
import { renderUnseen } from './swarm-unseen.js';
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
export const FRAMES = [1.4, 1.75, 3.5, 6.2, 9.8];
const frames = {};
for (const f of FRAMES) { const n = renderUnseen(f, {}).G, r = renderUnseen(f); frames[f] = { G: cells(r.G), N: cells(n), P: cells(r.P).map(x => x.slice(0, 46)) }; }
console.log(JSON.stringify(frames));
