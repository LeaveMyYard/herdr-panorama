// Turn 3: 46-col docked cockpit panel, four directions. Each screen = panel + stub of the docked agent chat.
// Reference copy for herdr-panorama: the operator picked 3a (vA, "Compact list"; 3b was used before); names follow
// the anonymised dataset in swarm-tui.js.
import { TH, DATA, H } from './swarm-tui.js';
import { blinkCol } from './swarm-smooth.js';
const { setFrame, Scr, LIVE, byUrg, isDone, curIdx, roleL, glyph, nq, trunc, fit, len, GL } = H;
const th = TH.dark, PW = 46, W = 120, HH = 45, HOV = '#171a1f';
let F = 0, DOCK = 'herdr core';
const PB = { done: ['#2c6b4c', '#e3f2e9'], run: ['#1f625c', '#d8f2ee'], rev: ['#2e4f86', '#dfe8f8'], q: ['#2a2e35', '#9aa0aa'], wait: ['#f2a93b', '#1b1203'], blk: ['#f2a93b', '#1b1203'], fail: ['#d8363e', '#ffffff'], cxl: ['#24272c', '#6b717a'] };
const ATT = st => st === 'wait' || st === 'blk' || st === 'fail';
const on = () => Math.floor(F / 2) % 2 === 0;
const gcol = st => ATT(st) ? blinkCol(th, th.st[st].t, F) : th.st[st].t;
const sl = segs => segs.reduce((a, g) => a + (g == null ? 0 : len(typeof g === 'string' ? g : g[0])), 0);
const RP = (s, xr, y, segs) => s.P(xr - sl(segs), y, segs);
const pod = (s, x, y, st, g) => { const [b, f] = PB[st]; s.P(x, y, [[' ' + g + ' ', f, 1, b]]); };
const bar = (s, y, h) => { for (let j = 0; j < h; j++) s.put(0, y + j, '▌', th.key); };
function rbox(s, x, y, w, h, col) { s.put(x, y, '╭' + '─'.repeat(w - 2) + '╮', col); for (let j = 1; j < h - 1; j++) { s.put(x, y + j, '│', col); s.put(x + w - 1, y + j, '│', col); } s.put(x, y + h - 1, '╰' + '─'.repeat(w - 2) + '╯', col); }
function edge(s, x, y, w, h, o, seed = 0) {
  if (o.st === 'q' || o.st === 'done') return;
  for (let j = 1; j < h - 1; j++) s.put(x, y + j, '┃', gcol(o.st));
  if (o.working) { const n = w - 2, p = (Math.floor(F) + seed * 7) % (n + 6) - 3; for (let i = 0; i < 4; i++) { const xx = p + i; if (xx >= 0 && xx < n) s.put(x + 1 + xx, y + h - 1, '━', th.st.run.t); } }
}
function oState(or) {
  const ags = or.threads.flatMap(t => t.agents), has = k => ags.some(a => a.st === k), r = ags.find(a => a.st === 'run'), q = nq(ags);
  let st, l;
  if (or.threads.every(isDone)) [st, l] = ['done', 'done'];
  else if (has('fail')) [st, l] = ['fail', 'failed'];
  else if (has('blk')) [st, l] = ['blk', 'blocked'];
  else if (r || has('rev')) [st, l] = [r ? 'run' : 'rev', 'working'];
  else if (q) [st, l] = ['wait', 'waiting'];
  else [st, l] = ['q', 'idle'];
  return { st, l, q, g: st === 'run' ? glyph(r) : GL[st], working: !!r || has('rev'), ags, live: ags.filter(LIVE), act: or.threads.filter(t => !isDone(t)), done: or.threads.filter(isDone) };
}
const stCol = o => ATT(o.st) ? th.st[o.st].t : o.working ? th.st.run.t : th.dim;
function tState(t) { if (isDone(t)) return { st: 'done', g: '✓' }; const u = t.agents.filter(LIVE).sort(byUrg)[0]; return u ? { st: u.st, g: glyph(u) } : { st: 'q', g: '○' }; }
// glyph rows keep one space between glyphs: some terminals draw ◐◑◒◓ wider than a cell (East Asian
// ambiguous width), and adjacent ones overlapped (the operator's report, ghostty)
const spaced = gs => gs.flatMap((g, i) => (i ? [' ', g] : [g]));
const agSegs = (ags, max = 8) => { const live = ags.filter(LIVE).sort(byUrg), fin = ags.length - live.length; const o = spaced(live.slice(0, max).map(a => [glyph(a), th.st[a.st].t, ATT(a.st)])); if (fin) o.push([' +' + fin + '✓', th.faint]); return o; };
const thSegs = o => spaced([...o.act.map(t => { const ts = tState(t); return [ts.g, th.st[ts.st].t, ATT(ts.st)]; }), ...o.done.map(() => ['✓', th.st.done.t])]);
const tItems = or => { const act = or.threads.filter(t => !isDone(t)), d = or.threads.length - act.length; return d ? [...act, { fold: d }] : act; };
const right = (o, extra = []) => [...extra, o.q ? ['?' + o.q, th.st.wait.t, 1] : null, o.q ? ' ' : null, [o.l, stCol(o)]];

function frame(nOrch = 4) {
  const s = new Scr(W, HH, th), all = DATA.flatMap(o => o.threads.flatMap(t => t.agents)), n = k => all.filter(a => a.st === k).length;
  for (let y = 0; y < HH; y++) s.put(PW, y, '│', th.line);
  s.P(1, 0, [['SWARM', th.dim, 1], ['  ' + nOrch + ' orchestrators', th.faint]]);
  RP(s, 45, 0, [['?' + nq(all), th.st.wait.t, 1], ' ', ['!' + n('blk'), th.st.blk.t, 1], ' ', ['✗' + n('fail'), th.st.fail.t, 1]]);
  s.hl(0, HH - 2, PW, th.line); s.put(PW, HH - 2, '┤', th.line);
  const K = th.key, f = th.faint;
  s.P(1, HH - 1, [['↑↓', K], [' move ', f], ['ret', K], [' open ', f], ['a', K], [' answer ', f], ['n', K], [' new ', f], ['pfx+spc', K], [' back', f]]);
  chat(s); return s;
}
function newAgent(s, y, hov) {
  if (hov) s.bg(1, y, 44, 1, HOV);
  s.P(2, y, [['+', th.key, 1], ' ', ['New agent', hov ? th.hi : th.dim]]); RP(s, 45, y, [['n', th.faint]]);
}
function chat(s) {
  const X = PW + 2, w = W - X - 1, L = (y, segs) => s.P(X, y, segs.map(g => typeof g === 'string' ? g : [trunc(g[0], w), g[1], g[2]]));
  L(0, [['herdr core', th.hi, 1], ['  tab 3 · coordinator · claude code', th.faint]]);
  s.hl(PW + 1, 1, W - PW - 1, th.line); s.put(PW, 1, '├', th.line);
  L(3, [['> ', th.dim], ['Fix the tab prefix stacking bug. Work in wt/tab-prefix.', th.fg]]);
  L(5, [['● ', th.st.run.t], ['Reproduced: renaming twice gives "[3] [3] build".', th.fg]]);
  L(6, [['  The prefix is written in rename() and again in render().', th.fg]]);
  L(7, [['  └ Read src/tui.py · 812 lines', th.faint]]);
  L(9, [['? ', th.st.wait.t, 1], ['2 questions before I edit src/tui.py:812', th.st.wait.t, 1]]);
  L(10, [['  1. Strip the "[3] " prefix on rename, or only at render time?', th.fg]]);
  L(11, [['  2. OK to update the tab-title snapshots in tests/test_tui.py?', th.fg]]);
  rbox(s, X, HH - 5, w + 1, 3, th.line);
  s.P(X + 2, HH - 4, [['> ', th.dim], [on() ? '█' : ' ', th.fg]]);
  L(HH - 2, [['  esc to interrupt · ? for shortcuts', th.faint]]);
}

// A · compact list, herdr-sidebar style
function vA() {
  const s = frame(); let y = 2;
  for (const or of DATA) {
    const o = oState(or), dock = or.name === DOCK, hov = or.name === 'Billing Migrations', items = dock ? tItems(or) : [];
    if (dock) { s.bg(0, y, PW, 2, th.sel); bar(s, y, 2 + items.length); } else if (hov) { s.bg(0, y, PW, 2, HOV); s.put(0, y, '›', th.dim); }
    s.P(2, y, [[String(or.tab), th.faint], ' ', [o.g, gcol(o.st), 1], ' ', [trunc(or.name, 22), dock || hov ? th.hi : th.fg, dock]]);
    RP(s, 45, y, right(o));
    s.P(6, y + 1, [...thSegs(o), ['  ' + o.act.length + ' active · ' + o.live.length + ' agents', th.faint]]);
    y += 2;
    items.forEach((t, i) => {
      s.put(3, y, i === items.length - 1 ? '└' : '├', th.line);
      if (t.fold) s.P(5, y, [['✓', th.st.done.t], ' ', [t.fold + ' done', th.faint]]);
      else { const ts = tState(t); s.P(5, y, [[ts.g, gcol(ts.st), ATT(ts.st)], ' ', [trunc(t.name, 24), th.fg]]); RP(s, 45, y, agSegs(t.agents, 4)); }
      y++;
    });
    y++;
  }
  newAgent(s, y); return s;
}

// B · cards with a pod row
function vB() {
  const s = frame(); let y = 2;
  for (const or of DATA) {
    const o = oState(or), dock = or.name === DOCK, hov = or.name === 'Billing Migrations', items = dock ? tItems(or) : [];
    const h = 4 + (items.length ? items.length + 1 : 0);
    if (dock) { s.bg(1, y, 44, h, th.nowBg); s.box(1, y, 44, h, th.key, true); s.P(4, y, [[' docked · tab ' + or.tab + ' ', th.key, 1]]); }
    else { if (hov) s.bg(1, y, 44, h, HOV); rbox(s, 1, y, 44, h, hov ? th.dim : th.line); }
    pod(s, 3, y + 1, o.st, o.g);
    s.P(7, y + 1, [[trunc(or.name, 20), dock || hov ? th.hi : th.fg, 1]]); RP(s, 43, y + 1, right(o));
    let x = 7; const live = o.live.slice().sort(byUrg);
    for (const a of live.slice(0, 7)) { pod(s, x, y + 2, a.st, glyph(a)); x += 4; }
    const fin = o.ags.length - live.length; if (fin) s.P(x, y + 2, [['+' + fin + ' ✓', th.faint]]);
    if (items.length) {
      s.put(1, y + 3, '┠' + '─'.repeat(42) + '┨', th.key);
      items.forEach((t, i) => {
        const yy = y + 4 + i;
        if (t.fold) { s.P(3, yy, [['✓', th.st.done.t], ' ', [t.fold + ' thread done', th.faint]]); return; }
        const ts = tState(t); s.P(3, yy, [[ts.g, gcol(ts.st), ATT(ts.st)], ' ', [trunc(t.name, 26), th.fg]]); RP(s, 43, yy, [[t.ph[curIdx(t)][0], th.dim]]);
      });
    }
    edge(s, 1, y, 44, h, o, or.tab);
    y += h + 1;
  }
  newAgent(s, y); return s;
}

// C · tree, docked orchestrator unfolds to threads → agents
function vC() {
  const s = frame(); let y = 2;
  for (const or of DATA) {
    const o = oState(or), dock = or.name === DOCK;
    if (!dock) {
      s.P(1, y, [['▸', th.faint], ' ', [o.g, gcol(o.st), 1], ' ', [trunc(or.name, 22), th.fg]]); RP(s, 45, y, right(o));
      s.P(5, y + 1, [...thSegs(o), ['  ' + o.act.length + ' threads · ' + o.live.length + ' agents', th.faint]]);
      y += 3; continue;
    }
    const y0 = y; s.bg(0, y, PW, 1, th.sel);
    s.P(1, y, [['▾', th.dim], ' ', [o.g, gcol(o.st), 1], ' ', [trunc(or.name, 22), th.hi, 1]]); RP(s, 45, y, right(o)); y++;
    const items = tItems(or);
    items.forEach((t, i) => {
      const last = i === items.length - 1, pre = last ? ' ' : '│';
      s.put(2, y, last ? '└─' : '├─', th.line);
      if (t.fold) { s.P(5, y, [['✓', th.st.done.t], ' ', [t.fold + ' thread done', th.faint]]); y++; return; }
      const ts = tState(t); s.P(5, y, [[ts.g, gcol(ts.st), ATT(ts.st)], ' ', [trunc(t.name, 26), th.fg]]); RP(s, 45, y, [[t.ph[curIdx(t)][0], th.dim]]); y++;
      const live = t.agents.filter(LIVE).sort(byUrg), fin = t.agents.length - live.length, rows = [...live, ...(fin ? [{ fold: fin }] : [])];
      rows.forEach((a, j) => {
        const hov = a.id === 'j1';
        if (hov) { s.bg(1, y, 45, 1, HOV); }
        s.put(2, y, pre, th.line); s.put(5, y, j === rows.length - 1 ? '└─' : '├─', th.line);
        if (a.fold) s.P(8, y, [['✓', th.st.done.t], ' ', [a.fold + ' finished', th.faint]]);
        else { s.P(8, y, [[glyph(a), gcol(a.st), ATT(a.st)], ' ', [fit(roleL(t, a.role), 11), hov ? th.hi : th.fg], [trunc(a.act, 24), hov ? th.fg : th.dim]]); }
        y++;
      });
    });
    bar(s, y0, y - y0); y++;
  }
  newAgent(s, y, true); return s;
}

// D · attention-first
function vD() {
  const s = frame(); let y = 2;
  const need = DATA.flatMap(or => or.threads.flatMap(t => t.agents)).filter(a => ATT(a.st)).sort(byUrg);
  s.P(1, y, [['NEEDS YOU', th.st.wait.t, 1], ['  ' + need.length + ' agents', th.faint]]); y += 2;
  for (const a of need) {
    const t = a.t, or = t.or, hov = a.id === 'c1', h = hov ? 3 : 2;
    if (hov) s.bg(1, y, 45, h, HOV);
    if (or.name === DOCK) bar(s, y, h);
    for (let j = 0; j < h; j++) s.put(1, y + j, '┃', gcol(a.st));
    pod(s, 3, y, a.st, GL[a.st]);
    s.P(7, y, [[trunc(or.name, 14), th.dim], [' › ', th.faint], [trunc(t.name.replace(/^(Bug|Worker|Task \d\/\d): /, ''), 18), hov ? th.hi : th.fg, 1]]);
    RP(s, 45, y, [[a.since || '', th.faint]]);
    const det = a.st === 'fail' ? a.err : a.st === 'blk' ? a.perm : a.qs[0] + (a.q > 1 ? ' (+' + (a.q - 1) + ')' : '');
    s.P(7, y + 1, [[trunc(det, 38), a.st === 'wait' ? th.fg : th.dim]]);
    if (hov) s.P(7, y + 2, [['a', th.key, 1], [' answer  ', th.dim], ['ret', th.key, 1], [' open  ', th.dim], ['x', th.key, 1], [' dismiss', th.dim]]);
    y += h + 1;
  }
  s.hl(1, y, 44, th.line); s.P(3, y, [[' ORCHESTRATORS ', th.dim]]); y += 2;
  for (const or of DATA) {
    const o = oState(or), dock = or.name === DOCK, items = dock ? tItems(or) : [];
    if (dock) { s.bg(0, y, PW, 1, th.sel); bar(s, y, 1 + items.length); }
    s.P(2, y, [[String(or.tab), th.faint], ' ', [o.g, th.st[o.st].t, 1], ' ', [trunc(or.name, 15), dock ? th.hi : th.fg, dock]]);
    s.P(22, y, agSegs(o.ags, 6).filter(g => g[1] !== th.faint));
    RP(s, 45, y, right(o)); y++;
    items.forEach((t, i) => {
      s.put(3, y, i === items.length - 1 ? '└' : '├', th.line);
      if (t.fold) s.P(5, y, [['✓', th.st.done.t], ' ', [t.fold + ' done', th.faint]]);
      else { const ts = tState(t); s.P(5, y, [[ts.g, th.st[ts.st].t, ATT(ts.st)], ' ', [trunc(t.name, 26), th.fg]]); RP(s, 45, y, [[t.ph[curIdx(t)][0], th.dim]]); }
      y++;
    });
  }
  y++; newAgent(s, y); return s;
}

export function renderDock(f = 0) {
  F = f; setFrame(Math.floor(f));
  return { K1: vA().rows(), K2: vB().rows(), K3: vC().rows(), K4: vD().rows() };
}

export const DK = { th, PW, W, HH, HOV, PB, ATT, on, gcol, sl, RP, pod, bar, rbox, edge, oState, stCol, tState, agSegs, thSegs, tItems, right, frame, newAgent, setF: f => { F = f; setFrame(Math.floor(f)); } };
