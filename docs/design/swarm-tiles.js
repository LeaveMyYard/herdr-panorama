// Reference copy for herdr-panorama (the operator picked 9e); names follow the anonymised dataset in swarm-tui.js.
// Turn 9: the docked 3a panel (46 cols) with 2a's coloured tiles. Same data as 3a (swarm-tui.js) plus one
// all-done orchestrator so every state shows: docked (herdr core, needs you), hover (Billing Migrations),
// failed, blocked, working, done.
// Running is the braille spinner everywhere (the operator's pick: cell-exact in ghostty), today's 3a included.
//   cur  today's 3a (swarm-dock.js vA): bare glyphs, one space between them
//   9a   a tile for the orchestrator's state glyph only
//   9b   tiles for every glyph on the entry: state tile + one tile per thread (a cell apart); done threads fold into one ✓n tile
//   9c   9b + the docked entry's thread list: a tile per thread; its agents stay bare glyphs
//   9d   9c with 1-cell tiles for the agents too
//   9e   9c, but the expanded entry drops its line-2 tiles (the list already shows them)
//   9e unseen  9e with turn 6's unseen marks: the entry's two lines pulse as a tint (as turn 6), and in the docked
//        list an unseen thread's tile (and the done fold's tile) pulses its BACKGROUND in the event colour, the same
//        3 s rhythm and never fully off (operator, Q115: "tile background pulses")
import { TH, DATA, H } from './swarm-tui.js';
import { blinkK } from './swarm-smooth.js';
const { setFrame, Scr, LIVE, byUrg, isDone, nq, trunc, len, GL } = H;
const th = TH.dark, PW = 46, HH = 45, HOV = '#171a1f';
let F = 0;
const DOCK = 'herdr core', HOVER = 'Billing Migrations';
const PB = { done: ['#2c6b4c', '#e3f2e9'], run: ['#1f625c', '#d8f2ee'], rev: ['#2e4f86', '#dfe8f8'], q: ['#2a2e35', '#9aa0aa'], wait: ['#f2a93b', '#1b1203'], blk: ['#f2a93b', '#1b1203'], fail: ['#d8363e', '#ffffff'], cxl: ['#24272c', '#6b717a'] };
const OFF = ['#2a2e35', '#4d535c']; // a needs-you tile in its blink-off phase
const ATT = st => st === 'wait' || st === 'blk' || st === 'fail';
const on = () => Math.floor(F / 2) % 2 === 0;
// smooth pulse: PULSE = 0..1 brightness (null = the 2-state blink)
let PULSE = null;
const hx = c => [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16));
const mix = (a, b, p) => { const A = hx(a), B = hx(b); return '#' + A.map((v, i) => Math.round(B[i] + (v - B[i]) * p).toString(16).padStart(2, '0')).join(''); };
// smooth animation (swarm-smooth.js): the blink eases with F (whole frames: on, on, off, off)
const gcol = st => !ATT(st) ? th.st[st].t : mix(th.st[st].t, th.faint, PULSE != null ? PULSE : blinkK(F));
const sl = segs => segs.reduce((a, g) => a + (g == null ? 0 : len(typeof g === 'string' ? g : g[0])), 0);
const RP = (s, xr, y, segs) => s.P(xr - sl(segs), y, segs);
const bar = (s, y, h) => { for (let j = 0; j < h; j++) s.put(0, y + j, '▌', th.key); };

// one more orchestrator, everything finished, so the done look is on screen
if (!DATA.some(o => o.name === 'Docs Site')) {
  const or = { name: 'Docs Site', tab: 5, fl: 'pr', threads: [] };
  const mk = (id, name, ags) => { const t = { id, name, ph: [['author', 'd', '08:00', '08:20'], ['merge', 'd', '08:40', '08:40']], or, agents: ags }; ags.forEach((a, i) => { a.t = t; a._n = 900 + i; }); return t; };
  or.threads.push(mk('d1x', 'PR #77 search index', [{ id: 'z1', role: 'author', st: 'done', phs: [0] }, { id: 'z2', role: 'reviewer', st: 'done', phs: [0] }]));
  or.threads.push(mk('d2x', 'PR #79 footer links', [{ id: 'z3', role: 'author', st: 'done', phs: [0] }]));
  DATA.push(or);
}

// glyphs: the one table in swarm-tui.js (GL; the braille SPIN through H.glyph), as the app's model.py
const mkG = () => a => a.st === 'run' ? H.glyph(a) : GL[a.st];
function oState(or, G) {
  const ags = or.threads.flatMap(t => t.agents), has = k => ags.some(a => a.st === k), r = ags.find(a => a.st === 'run'), q = nq(ags);
  let st, l;
  if (or.threads.every(isDone)) [st, l] = ['done', 'done'];
  else if (has('fail')) [st, l] = ['fail', 'failed'];
  else if (has('blk')) [st, l] = ['blk', 'blocked'];
  else if (r || has('rev')) [st, l] = [r ? 'run' : 'rev', 'working'];
  else if (q) [st, l] = ['wait', 'needs you']; // turn 8 (operator, Q109): 'waiting' means background work
  else [st, l] = ['q', 'idle'];
  return { st, l, q, g: st === 'run' ? G(r) : GL[st], working: !!r || has('rev'), ags, live: ags.filter(LIVE), act: or.threads.filter(t => !isDone(t)), done: or.threads.filter(isDone) };
}
const stCol = o => ATT(o.st) ? th.st[o.st].t : o.working ? th.st.run.t : th.dim;
function tState(t, G) { if (isDone(t)) return { st: 'done', g: '✓' }; const u = t.agents.filter(LIVE).sort(byUrg)[0]; return u ? { st: u.st, g: G(u) } : { st: 'q', g: '○' }; }
const right = o => [o.q ? ['?' + o.q, th.st.wait.t, 1] : null, o.q ? ' ' : null, [o.l, stCol(o)]];
const tItems = or => { const act = or.threads.filter(t => !isDone(t)), d = or.threads.length - act.length; return d ? [...act, { fold: d }] : act; };

// tiles: 2a's pod (' g ', 3 cells) and a 1-cell one; needs-you tiles blink like the bare glyphs do
const tileC = st => { if (!ATT(st)) return PB[st]; const k = PULSE != null ? PULSE : blinkK(F); return [mix(PB[st][0], OFF[0], k), mix(PB[st][1], OFF[1], k)]; };
const tile = (st, g) => { const [b, f] = tileC(st); return [' ' + g + ' ', f, 1, b]; };
const mini = (st, g) => { const [b, f] = tileC(st); return [g, f, 1, b]; };
const spaced = gs => gs.flatMap((g, i) => (i ? [' ', g] : [g]));
const bare = (st, g) => [g, th.st[st].t, ATT(st)];
const bareB = (st, g) => [g, gcol(st), ATT(st)]; // blinking bare glyph (state / thread list), as 3a

// ── unseen (turn 6, swarm-unseen.js): the same sample marks, rank, period and strengths
const UNSEEN = { b3: 'fail', c1: 'wait', a5: 'done', i3: 'wait', k2: 'done' };
const RANK = { fail: 0, wait: 1, done: 2 };
const PERIOD = 12; // frames of 250 ms: one slow pulse every 3 s
const toward = (a, b, k) => '#' + hx(a).map((v, i) => Math.round(v + (hx(b)[i] - v) * k).toString(16).padStart(2, '0')).join('');
const wave = () => (1 - Math.cos(2 * Math.PI * (F % PERIOD) / PERIOD)) / 2; // 0 .. 1 .. 0
function markOf(marks, ags) { // the most urgent unseen event among these agents; a finished one is quiet (a roll-up)
  const evs = ags.map(a => marks[a.id]).filter(Boolean).sort((x, y) => RANK[x] - RANK[y]);
  return evs.length ? { ev: evs[0], quiet: evs[0] === 'done' } : null;
}
// a tile whose background pulses toward the event colour: full 35..100 %, quiet 15..45 % (turn 6's pulseCol)
const pulseTile = (t, m) => m ? [t[0], t[1], t[2], toward(t[3], th.st[m.ev].t, m.quiet ? 0.15 + 0.3 * wave() : 0.35 + 0.65 * wave())] : t;

// ── the panel; V = { stTile, rowTiles, listTiles, agMini, hideRowOnList, unseen }
function panel(V) {
  const U = V.unseen || {};
  const G = mkG(), s = new Scr(PW, HH, th), all = DATA.flatMap(o => o.threads.flatMap(t => t.agents)), n = k => all.filter(a => a.st === k).length;
  s.P(1, 0, [['SWARM', th.dim, 1], ['  ' + DATA.length + ' orchestrators', th.faint]]);
  RP(s, 45, 0, [['?' + nq(all), th.st.wait.t, 1], ' ', ['!' + n('blk'), th.st.blk.t, 1], ' ', ['✗' + n('fail'), th.st.fail.t, 1]]);
  let y = 2;
  const X = V.stTile ? 8 : 6; // where the name and line 2 start
  for (const or of DATA) {
    const o = oState(or, G), dock = or.name === DOCK, hov = or.name === HOVER, items = dock ? tItems(or) : [];
    const m = markOf(U, o.ags), base = dock ? th.sel : hov ? HOV : th.bg; // unseen: the entry's two lines pulse as a tint
    if (dock || hov || m) s.bg(0, y, PW, 2, m ? toward(base, th.st[m.ev].t, m.quiet ? 0.04 + 0.1 * wave() : 0.08 + 0.22 * wave()) : base);
    if (dock) bar(s, y, 2 + items.length); else if (hov) s.put(0, y, '›', th.dim);
    const name = [trunc(or.name, 22), dock || hov ? th.hi : o.st === 'done' ? th.dim : th.fg, dock];
    if (V.stTile) s.P(2, y, [[String(or.tab), th.faint], ' ', tile(o.st, o.g), ' ', name]);
    else s.P(2, y, [[String(or.tab), th.faint], ' ', bareB(o.st, o.g), ' ', name]);
    RP(s, 45, y, right(o));
    // line 2: one glyph (or tile) per thread, then counts
    let segs;
    if (V.rowTiles) {
      const ts = o.act.map(t => { const x = tState(t, G); return tile(x.st, x.g); });
      if (o.done.length) ts.push(tile('done', '✓' + (o.done.length > 1 ? o.done.length : '')));
      segs = spaced(ts);
    } else segs = spaced([...o.act.map(t => { const x = tState(t, G); return bare(x.st, x.g); }), ...o.done.map(() => bare('done', '✓'))]);
    const cnt = `${o.act.length} active · ${o.live.length} agents`;
    if (V.hideRowOnList && items.length) s.P(X, y + 1, [[cnt, th.faint]]);
    else s.P(X, y + 1, [...segs, ['  ' + cnt, th.faint]]);
    y += 2;
    items.forEach((t, i) => {
      s.put(3, y, i === items.length - 1 ? '└' : '├', th.line);
      if (t.fold) {
        if (V.listTiles) s.P(5, y, [pulseTile(tile('done', '✓'), markOf(U, o.done.flatMap(x => x.agents))), ' ', [t.fold + ' done', th.faint]]);
        else s.P(5, y, [['✓', th.st.done.t], ' ', [t.fold + ' done', th.faint]]);
      } else {
        const x = tState(t, G);
        if (V.listTiles) s.P(5, y, [pulseTile(tile(x.st, x.g), markOf(U, t.agents)), ' ', [trunc(t.name, 22), th.fg]]);
        else s.P(5, y, [bareB(x.st, x.g), ' ', [trunc(t.name, 24), th.fg]]);
        const live = t.agents.filter(LIVE).sort(byUrg), fin = t.agents.length - live.length;
        const ag = spaced(live.slice(0, 4).map(a => (V.agMini ? mini : bare)(a.st, G(a))));
        if (fin) ag.push([' +' + fin + '✓', th.faint]);
        RP(s, 45, y, ag);
      }
      y++;
    });
    y++;
  }
  s.P(2, y, [['+', th.key, 1], ' ', ['New agent', th.dim]]); RP(s, 45, y, [['n', th.faint]]);
  s.hl(0, HH - 2, PW, th.line);
  s.P(1, HH - 1, [['↑↓', th.key], [' move ', th.faint], ['ret', th.key], [' open ', th.faint], ['a', th.key], [' answer ', th.faint], ['n', th.key], [' new ', th.faint], ['pfx+spc', th.key], [' back', th.faint]]);
  return s;
}

export function renderTiles(f = 0) {
  F = f; setFrame(Math.floor(f)); // f may be fractional: colours glide, spinners step
  return {
    P0: panel({}).rows(),
    PA: panel({ stTile: 1 }).rows(),
    PB: panel({ stTile: 1, rowTiles: 1 }).rows(),
    PC: panel({ stTile: 1, rowTiles: 1, listTiles: 1 }).rows(),
    PE: panel({ stTile: 1, rowTiles: 1, listTiles: 1, hideRowOnList: 1 }).rows(),
    PD: panel({ stTile: 1, rowTiles: 1, listTiles: 1, agMini: 1 }).rows(),
    PU: panel({ stTile: 1, rowTiles: 1, listTiles: 1, hideRowOnList: 1, unseen: UNSEEN }).rows(),
  };
}

// 9f: 9e with a smooth pulse instead of the on/off blink. ms = wall clock; the spinner keeps its 250 ms step.
export function renderPulse(ms) {
  F = Math.floor(ms / 250); setFrame(F);
  const ph = (ms % 1400) / 1400; PULSE = 0.5 + 0.5 * Math.cos(ph * 2 * Math.PI); PULSE = PULSE * PULSE * (3 - 2 * PULSE);
  const rows = panel({ stTile: 1, rowTiles: 1, listTiles: 1, hideRowOnList: 1 }).rows();
  PULSE = null; return rows;
}
