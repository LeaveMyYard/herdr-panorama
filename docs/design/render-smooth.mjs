// Render the smooth animation from the reference JS (swarm-smooth.js) between whole frames, where the
// pulses and the attention blink are mid-way: turn 6's unseen 2a graph and turn 9's 9e docked panel.
// Usage: node render-smooth.mjs json > smooth.json
//   G the 160x45 graph with the unseen sample marks, N the same with none (swarm-unseen.js);
//   E the 9e panel, U the same with the unseen marks (swarm-tiles.js); the 46 panel columns
import { renderUnseen } from './swarm-unseen.js';
import { renderTiles } from './swarm-tiles.js';
const cells = rows => rows.map(row => { const out = []; for (const u of row.r) { const t = Array.from(u.t); const n = parseInt(u.st.width); for (let i = 0; i < n; i++) out.push([t[i] ?? ' ', u.st.color, u.st.background === 'transparent' ? null : u.st.background, u.st.fontWeight === 700]); } return out; });
export const FRAMES = [1.4, 1.75, 3.5, 6.2, 9.8];
const frames = {};
for (const f of FRAMES) {
  const n = renderUnseen(f, {}).G, r = renderUnseen(f), t = renderTiles(f);
  frames[f] = { G: cells(r.G), N: cells(n), E: cells(t.PE).map(x => x.slice(0, 46)), U: cells(t.PU).map(x => x.slice(0, 46)) };
}
console.log(JSON.stringify(frames));
