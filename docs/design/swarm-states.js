// Reference copy for herdr-panorama (names follow the anonymised dataset in swarm-tui.js).
// Turn 8: two new orchestrator/agent states, "waiting" and "stale". A copy of 2a's wide renderer
// (swarm-unseen.js without the pulse) and of 3a (swarm-dock.js vA), with these additions:
//   - waiting ⠁ (lavender): herdr says idle, but background work it started is still in flight:
//     a Bash run_in_background (or a Bash that timed out into the background), or a Monitor watch.
//     One braille dot circles the cell, a step per second (⠁⠈⠐⠠⢀⡀⠄⠂; not the clocks ◴◵◶◷: ghostty
//     takes those from a wider fallback font, while it draws braille itself, cell-exact). Steady lavender left edge,
//     no sweep (nothing is thinking), no blink (nothing needs you).
//   - stale ◌ (grey): idle or all done, and nothing happened for longer than stale_after (12h).
//     The card keeps a plain border; name and label go faint; the label shows the age.
//   - A waiting orchestrator's background tasks are a "background" card in the thread column (2a)
//     and tiles in the docked entry's list (the 9e panel, swarm-tiles.js, since turn 9). A waiting agent lists its own in its expanded card.
// Agent cards are 44 columns wide and do not stretch, as the shipped 2a does since turn 5.
import { TH, DATA as BASE, H } from './swarm-tui.js';
import { blinkK, blinkCol, mix } from './swarm-smooth.js'; // smooth animation: F may be fractional
let F = 0;
const { setFrame, Scr, LIVE: LIVE0, isDone, curIdx, roleL, glyph: glyph0, lastPh, fit, trunc, wrap, tm, dur, NOW, FL, LBL: LBL0, mark, len, GL: GL0, nq } = H;

// ── the two new states: colours sit beside TH.dark.st; glyphs beside GL
const ST2 = { bg: { t: '#b39ddb', pf: '#e6def8', pb: '#3b3159' }, stale: { t: '#646a73', pf: '#6b717a', pb: '#1b1e23' } };
const th = { ...TH.dark, st: { ...TH.dark.st, ...ST2 } };
const CLOCK = ['⠁', '⠈', '⠐', '⠠', '⢀', '⡀', '⠄', '⠂'];
const GL = { ...GL0, bg: '⠁', stale: '◌' };
const LBL = { ...LBL0, bg: 'waiting', stale: 'stale' };
const ORDER = ['fail', 'blk', 'wait', 'rev', 'run', 'bg', 'q', 'done', 'cxl'];
const byUrg = (a, b) => ORDER.indexOf(a.st) - ORDER.indexOf(b.st);
const LIVE = a => LIVE0(a);
const clock = (seed = 0) => CLOCK[(Math.floor(F / 4) + seed) % CLOCK.length]; // one step per second
const glyph = a => a.st === 'bg' ? clock(a._n || 0) : glyph0(a);
const ago = m => m >= 2880 ? `${Math.floor(m / 1440)}d` : m >= 60 ? `${Math.floor(m / 60)}h` : `${m}m`;
const STALE_AFTER = 12 * 60; // minutes; config stale_after = "12h"

// ── sample data: the four orchestrators of the earlier turns, PR #214's author n1 waiting on
// its CI watch, plus four main-session-only orchestrators (no subagents): two waiting, two stale.
const bgT = (kind, desc, since) => ({ kind, desc, since });
const DATA = BASE.map(or => ({ ...or, threads: or.threads.map(t => ({ ...t, agents: t.agents.map(a => ({ ...a })) })) }));
let _n = 100;
for (const or of DATA) for (const t of or.threads) { t.or = or; for (const a of t.agents) { a.t = t; a._n = a._n ?? _n++; } }
const n1 = DATA.flatMap(o => o.threads).find(t => t.id === 'p214').agents.find(a => a.id === 'n1');
Object.assign(n1, { st: 'bg', act: 'turn ended · 1 task in background', since: '3m',
  bg: [bgT('Bash', 'Watch CI · gh run watch 4242', '10:01')],
  msg: 'Fix r1 pushed. Waiting for the 22 CI jobs before reporting back.' });
DATA.push(
  { name: 'Deploy Watch', tab: 5, fl: 'co', last: 3, threads: [], bg: [
    bgT('Bash', 'Wait for the console image to roll out on dev', '09:50'),
    bgT('Monitor', 'dev rollout events · argocd sync', '10:01')] },
  { name: 'Nightly Build', tab: 6, fl: 'co', last: 14 * 60 + 2, threads: [], bg: [
    bgT('Bash', 'Wait for the nightly build artefacts', '19:02')], bgAge: 15 * 60 + 2 },
  { name: 'Metrics Export', tab: 7, fl: 'co', last: 15 * 60 + 12, threads: [] },
  { name: 'Infra Probe', tab: 8, fl: 'pr', last: 39 * 60 + 18, threads: [] },
);
// the earlier four were last active minutes ago
for (const or of DATA) if (or.last == null) or.last = 1;

// ── state: the design's stOf with two additions (attention > working > reviewing > waiting > stale | idle | all done)
const agSt = a => ({ st: a.st, working: a.st === 'run' });
const thSt = t => stOf(t.agents, isDone(t));
function orchSt(or) {
  const ags = or.threads.flatMap(t => t.agents), o = stOf(ags, or.threads.every(isDone), or);
  return o;
}
function stOf(ags, allDone, or) {
  const has = k => ags.some(a => a.st === k), bg = or ? (or.bg || []).length : 0;
  for (const [k, g, l] of [['fail', '✗', 'failed'], ['blk', '!', 'blocked'], ['wait', '?', 'needs you']]) if (has(k)) {
    const r = ags.find(a => a.st === 'run'); return { st: k, g, l: l + (r ? ' · working' : ''), working: !!r };
  }
  const r = ags.find(a => a.st === 'run'); if (r) return { st: 'run', g: glyph(r), l: 'working', working: true };
  if (has('rev')) return { st: 'rev', g: '»', l: 'reviewing' };
  if (bg || has('bg')) { const n = bg + ags.filter(a => a.st === 'bg').length; return { st: 'bg', g: clock(), l: 'waiting', n, age: or && or.bgAge }; }
  if (or && or.last > STALE_AFTER) return { st: 'stale', g: '◌', l: 'stale ' + ago(or.last) };
  if (allDone && ags.length) return { st: 'done', g: '✓', l: 'all done' };
  return { st: 'q', g: '○', l: 'idle' };
}
const QUIET = st => st === 'q' || st === 'done' || st === 'stale' || st === 'cxl';

// ── 2a pieces (from swarm-unseen.js)
const PB = { done: ['#2c6b4c', '#e3f2e9'], run: ['#1f625c', '#d8f2ee'], rev: ['#2e4f86', '#dfe8f8'], q: ['#2a2e35', '#9aa0aa'], wait: ['#f2a93b', '#1b1203'], blk: ['#f2a93b', '#1b1203'], fail: ['#d8363e', '#ffffff'], cxl: ['#24272c', '#6b717a'], next: ['#1c1f24', '#454b54'],
  bg: [ST2.bg.pb, ST2.bg.pf], stale: [ST2.stale.pb, ST2.stale.pf] };
function pod(s, x, y, st, g, sel) { const [b, f] = PB[st]; s.P(x, y, [[' ' + g + ' ', f, 1, b]]); if (sel) { s.put(x - 1, y, '[', s.th.hi, undefined, 1); s.put(x + 3, y, ']', s.th.hi, undefined, 1); } }
const apod = (s, x, y, a, sel) => pod(s, x, y, a.st, glyph(a), sel);
function rbox(s, x, y, w, h, col) { s.put(x, y, '╭' + '─'.repeat(w - 2) + '╮', col); for (let j = 1; j < h - 1; j++) { s.put(x, y + j, '│', col); s.put(x + w - 1, y + j, '│', col); } s.put(x, y + h - 1, '╰' + '─'.repeat(w - 2) + '╯', col); }
function tags(s, x, y, w, list) { let xx = x + w - 2; for (const t of list.filter(Boolean).reverse()) { xx -= len(t) + 2; s.P(xx, y, [[' ' + t + ' ', s.th.dim]]); xx -= 1; } }
function marksRight(s, xr, y, t) { const mk = mark(s.th, t); let w = mk.reduce((a, m) => a + len(m[0]) + 1, 0) - 1; let x = xr - w; mk.forEach(m => { x = s.P(x, y, [m]) + 1; }); return Math.max(0, w); }
const BX = { 3: '│', 12: '─', 10: '╭', 9: '╰', 6: '╮', 5: '╯', 11: '├', 7: '┤', 14: '┬', 13: '┴', 15: '┼', 1: '│', 2: '│', 4: '─', 8: '─' };
const XB = { '│': 3, '─': 12, '╭': 10, '╰': 9, '╮': 6, '╯': 5, '┌': 10, '└': 9, '┐': 6, '┘': 5, '├': 11, '┤': 7, '┬': 14, '┴': 13, '┼': 15 };
function jn(s, x, y, bits, col) {
  if (y < 0 || y >= s.h || x < 0 || x >= s.w) return;
  const c = s.c[y][x], had = XB[c.ch];
  if (c.ch === '┃') { if (bits === 4) c.ch = '┨'; else if (bits === 8) c.ch = '┠'; return; }
  if (had != null) c.ch = BX[had | bits]; else { c.ch = BX[bits]; c.fg = col; c.bg = null; c.b = 0; }
}
function link(s, fx, fy, tx, toX, ys) {
  const c = s.th.line;
  jn(s, fx, fy, 8, c); for (let x = fx + 1; x < tx; x++) jn(s, x, fy, 12, c);
  const all = [fy, ...ys], y0 = Math.min(...all), y1 = Math.max(...all);
  for (let y = y0; y <= y1; y++) jn(s, tx, y, (y > y0 ? 1 : 0) | (y < y1 ? 2 : 0) | (y === fy ? 4 : 0) | (ys.includes(y) ? 8 : 0), c);
  for (const y of ys) { for (let x = tx + 1; x < toX; x++) jn(s, x, y, 12, c); jn(s, toX, y, 4, c); }
}
function podsRow(s, x, y, t, o, max) {
  const ags = t.agents.slice().sort(byUrg), live = ags.filter(LIVE), fin = ags.filter(a => !LIVE(a));
  const room = Math.max(0, max - live.length), showF = fin.length <= room ? fin : fin.slice(0, Math.max(0, room - 1));
  for (const a of [...live, ...showF]) { apod(s, x, y, a, o.selA === a.id); x += 4; }
  if (showF.length < fin.length) s.P(x, y, [['+' + (fin.length - showF.length) + ' done', s.th.faint]]);
}
function phaseGlyph(t, i) {
  const p = t.ph[i];
  if (p[1] === 'd') return ['✓', th.st.done.t];
  if (p[1] === 'x') return ['·', th.faint];
  if (p[1] === 'f') return ['✗', th.st.fail.t];
  const u = t.agents.filter(a => LIVE(a) && lastPh(a) === i).sort(byUrg)[0];
  return u ? [glyph(u), th.st[u.st].t] : ['⠋', th.st.run.t];
}
function stepLine(s, x, y, t, i, w) {
  const p = t.ph[i], cur = p[1] === 'n' || p[1] === 'f', [g, gc] = phaseGlyph(t, i);
  const time = p[1] === 'x' ? '' : p[3] ? `${p[2]}–${p[3]}` : `since ${p[2]} · ${dur(NOW - tm(p[2]))}`;
  const xx = s.P(x, y, [[g, gc, 1], ' ', [fit(p[0], 13), cur ? th.hi : p[1] === 'x' ? th.faint : th.fg, cur], [fit(time, 20), cur ? th.fg : th.faint]]);
  if (p[4] && xx < x + w - 2) s.P(xx, y, [[trunc(p[4], x + w - xx), th.dim]]);
}
// one background task: clock glyph, kind, description, since + age (age from the task's own start)
function bgLine(s, x, y, b, w, i) {
  const since = b.age != null ? b.age : NOW - tm(b.since), when = dur(since < 0 ? since + 1440 : since);
  s.P(x, y, [[clock(i + 1), th.st.bg.t, 1], ' ', [fit(b.kind, 8), th.dim], [fit(b.desc, w - 10 - len(when) - 1), th.fg]]);
  s.P(x + w - len(when), y, [[when, th.dim]]);
}
function hdr(s, o) {
  s.bg(0, 0, s.w, 1, th.hdr);
  let x = s.P(1, 0, [['herdr', th.hi, 1], [' panorama', th.dim]]) + 4;
  for (const [k, l] of [['all', 'all'], ['running', 'running'], ['needs', 'needs me']]) { const on = (o.f || 'all') === k; x = s.P(x, 0, [[l, on ? th.hi : th.faint, on]]) + 3; }
  const segs = [['? 3', th.st.wait.t, 1], '  ', ['! 1', th.st.blk.t, 1], '  ', ['✗ 1', th.st.fail.t, 1], '    ', ['10:04', th.dim]];
  const L = segs.reduce((a, g) => a + len(typeof g === 'string' ? g : g[0]), 0); s.P(s.w - L - 1, 0, segs);
}
function legend(s, y) {
  let x = 1;
  const L = [['done', '✓', 'done'], ['run', '⠋', 'running'], ['rev', '»', 'review'], ['bg', '⠁', 'waiting'], ['q', '○', 'queued'], ['stale', '◌', 'stale'], ['wait', '?', 'question'], ['blk', '!', 'blocked'], ['fail', '✗', 'failed']];
  for (const [k, g, l] of L) { pod(s, x, y, k, g); x = s.P(x + 4, y, [[l, th.faint]]) + 3; }
}
function keys(s, list) { const y = s.h - 1; let x = 1; for (const [k, l] of list) x = s.P(x, y, [[k, th.fg, 1], [' ' + l, th.faint]]) + 3; }
function agentDetailLines(s, a, W) {
  const out = [];
  out.push([['now  ', th.faint], [trunc(a.act, W - 5), th.fg]]);
  (a.bg || []).forEach(() => out.push(null)); // background rows are drawn by bgLine
  if (a.msg) wrap('“' + a.msg + '”', W).slice(0, 2).forEach(l => out.push([[l, th.dim]]));
  return out;
}
function actions(s, x, y, a) {
  let xx = x; for (const [k, l, on] of [['g', 'open pane', 1], ['t', 'transcript', 1], ['a', 'answer', 0]]) xx = s.P(xx, y, [[k, on ? th.key : th.faint, 1], [' ' + l, on ? th.fg : th.faint]]) + 3;
}
function edge(s, x, y, w, h, os, seed = 0) {
  if (!os || QUIET(os.st)) return;
  const c = th.st[os.st].t, att = os.st === 'wait' || os.st === 'blk' || os.st === 'fail', col = att ? blinkCol(th, c, F) : c;
  for (let j = 1; j < h - 1; j++) s.put(x, y + j, '┃', col);
  if (os.working) { const n = w - 2, p = (Math.floor(F) + seed * 7) % (n + 6) - 3; for (let i = 0; i < 4; i++) { const xx = p + i; if (xx >= 0 && xx < n) s.put(x + 1 + xx, y + h - 1, '━', th.st.run.t); } }
}

// filters: running = active threads, herdr working, or waiting (background work in flight); stale and
// idle are not running. needs me = attention only; waiting and stale never need you.
function showOrch(or, f) {
  const st = orchSt(or).st;
  if (f === 'running') return or.threads.some(t => !isDone(t)) || st === 'bg';
  if (f === 'needs') return st === 'fail' || st === 'blk' || st === 'wait';
  return true;
}
// ───────── 2a · Graph, 160 × h
function renderGraphS(w, h, o) {
  const s = new Scr(w, h, th);
  hdr(s, o);
  const OX = 2, OW = 28, TX = 36, TW = 50, AX = 93, AW = 44;
  let y = 3, selY = 0;
  for (const or of DATA.filter(x => showOrch(x, o.f))) {
    const sel = or.name === o.orch;
    const ags = or.threads.flatMap(t => t.agents), os = orchSt(or), quiet = QUIET(os.st), stale = os.st === 'stale';
    rbox(s, OX, y, OW, 4, sel ? th.dim : th.line);
    edge(s, OX, y, OW, 4, os);
    const q = ags.reduce((a, g) => a + (g.st === 'wait' ? g.q : 0), 0), b = ags.some(g => g.st === 'blk'), f = ags.some(g => g.st === 'fail');
    pod(s, OX + 2, y + 1, os.st, os.g);
    s.P(OX + 6, y + 1, [[trunc(or.name, OW - 13), sel ? th.hi : stale ? th.faint : quiet ? th.dim : th.fg, 1]]);
    const mk = [f && os.st !== 'fail' ? ['✗', th.st.fail.t, 1] : null, b && os.st !== 'blk' ? ['!', th.st.blk.t, 1] : null, q ? ['?' + q, th.st.wait.t, 1] : null].filter(Boolean);
    let mx = OX + OW - 2 - mk.reduce((a, m) => a + len(m[0]) + 1, -1); mk.forEach(m => { mx = s.P(mx, y + 1, [m]) + 1; });
    const nl = ags.filter(LIVE).length, na = or.threads.filter(t => !isDone(t)).length;
    const lab = os.st === 'bg' && os.age ? os.l + ' ' + ago(os.age) : os.l;
    const sl = trunc(lab, OW - 8);
    const rest = os.st === 'bg' ? trunc(` · ${os.n} bg`, OW - 8 - len(sl)) : quiet ? '' : trunc(` · ${na}/${nl}`, OW - 8 - len(sl));
    s.P(OX + 6, y + 2, [[sl, stale ? th.st.stale.t : quiet ? th.faint : th.st[os.st].t, !quiet], [rest, th.dim]]);
    tags(s, OX, y + 3, OW, [FL[or.fl].key]);
    if (sel) selY = y + 1;
    y += 5;
  }
  const or = DATA.find(d => d.name === o.orch);
  let ty = 3; const anchors = []; let selAnchor = 0;
  // the orchestrator's own background tasks: one card above its threads
  if ((or.bg || []).length) {
    const hh = 3 + or.bg.length, sel = o.selT === 'bg:';
    rbox(s, TX, ty, TW, hh, sel ? th.dim : th.line);
    edge(s, TX, ty, TW, hh, { st: 'bg' });
    s.P(TX + 2, ty + 1, [[clock(), th.st.bg.t, 1], ' ', ['background', sel ? th.hi : th.fg, 1], [`  ${or.bg.length} task${or.bg.length > 1 ? 's' : ''} · main session`, th.dim]]);
    or.bg.forEach((bt, i) => bgLine(s, TX + 4, ty + 2 + i, { ...bt, age: or.bgAge }, TW - 7, i));
    tags(s, TX, ty + hh - 1, TW, ['turn ended ' + dur(or.last) + ' ago']); // since its last activity
    anchors.push(ty + 1); if (sel) selAnchor = ty + 1;
    ty += hh + 1;
  }
  for (const t of or.threads.filter(t => !isDone(t))) {
    const sel = t.id === o.selT, hh = sel ? 6 + t.ph.length : 5, ci = curIdx(t), p = t.ph[ci], [g, gc] = phaseGlyph(t, ci);
    rbox(s, TX, ty, TW, hh, sel ? th.dim : th.line);
    edge(s, TX, ty, TW, hh, thSt(t), anchors.length + 1);
    const mw = marksRight(s, TX + TW - 2, ty + 1, t);
    s.P(TX + 2, ty + 1, [[g, gc, 1], ' ', [trunc(t.name, TW - 7 - mw), sel ? th.hi : th.fg, 1]]);
    s.P(TX + 4, ty + 2, [[p[0] + (p[2] ? ' · ' + dur(NOW - tm(p[2])) : '') + (t.pct != null ? ` · ${t.pct}%` : ''), th.dim]]);
    podsRow(s, TX + 4, ty + 3, t, o, 9);
    if (sel) t.ph.forEach((_, i) => stepLine(s, TX + 4, ty + 5 + i, t, i, TW - 6));
    tags(s, TX, ty + hh - 1, TW, [t.ph[0][2] ? dur(NOW - tm(t.ph[0][2])) : 'queued', t.wt]);
    anchors.push(ty + 1); if (sel) selAnchor = ty + 1;
    ty += hh + 1;
  }
  const dn = or.threads.filter(isDone);
  if (dn.length) { rbox(s, TX, ty, TW, 3, th.line); s.P(TX + 2, ty + 1, [['✓', th.st.done.t, 1], [` ${dn.length} complete`, th.fg], ['  ' + trunc(dn.map(t => t.name).join(', '), TW - 18), th.faint]]); anchors.push(ty + 1); }
  if (anchors.length) link(s, OX + OW - 1, selY, 32, TX, anchors);
  const t = or.threads.find(x => x.id === o.selT);
  if (t) {
    const ags = t.agents.slice().sort(byUrg), live = t.agents.filter(LIVE), fin = ags.filter(a => !LIVE(a));
    let ay = 3; const aAnch = [];
    for (const a of live) {
      const sel = a.id === o.selA;
      if (!sel) {
        rbox(s, AX, ay, AW, 3, th.line); edge(s, AX, ay, AW, 3, agSt(a), a._n); apod(s, AX + 2, ay + 1, a);
        const x2 = s.P(AX + 6, ay + 1, [[roleL(t, a.role), th.fg, 1], [' ' + a.id, th.faint], [' · ' + a.model, th.dim]]);
        s.P(x2 + 3, ay + 1, [[trunc(a.act, AX + AW - 3 - x2 - 3), th.faint]]);
        tags(s, AX, ay + 2, AW, [a.age, `${a.tools} calls`]); aAnch.push(ay + 1); ay += 4; continue;
      }
      const lines = agentDetailLines(s, a, AW - 6), hh = 6 + lines.length;
      rbox(s, AX, ay, AW, hh, th.hi); edge(s, AX, ay, AW, hh, agSt(a), a._n); apod(s, AX + 2, ay + 1, a, false);
      s.P(AX + 6, ay + 1, [[roleL(t, a.role), th.hi, 1], [' ' + a.id, th.faint], [' · ' + a.model, th.dim]]);
      const st = LBL[a.st] + (a.since ? ' ' + a.since : ''); s.P(AX + AW - 2 - len(st), ay + 1, [[st, th.st[a.st].t, 1]]);
      let bi = 0;
      lines.forEach((l, i) => { if (l) s.P(AX + 4, ay + 3 + i, l); else bgLine(s, AX + 4, ay + 3 + i, a.bg[bi], AW - 7, bi++); });
      actions(s, AX + 4, ay + hh - 2, a);
      tags(s, AX, ay + hh - 1, AW, [a.age, `${a.tools} calls`, t.ph[lastPh(a)][0]]);
      aAnch.push(ay + 1); ay += hh + 1;
    }
    if (fin.length) {
      rbox(s, AX, ay, AW, 3, th.line); let x = AX + 2; for (const a of fin.slice(0, 5)) { apod(s, x, ay + 1, a); x += 4; }
      const roles = {}; fin.forEach(a => { const r = roleL(t, a.role); roles[r] = (roles[r] || 0) + 1; });
      s.P(x + 1, ay + 1, [[trunc(`${fin.length} finished · ` + Object.entries(roles).map(([r, n]) => n > 1 ? `${r} ×${n}` : r).join(', '), AX + AW - x - 5), th.faint]]); s.put(AX + AW - 3, ay + 1, '▸', th.faint);
      aAnch.push(ay + 1);
    }
    link(s, TX + TW - 1, selAnchor, 89, AX, aAnch);
  }
  legend(s, h - 3);
  keys(s, [['↑↓', 'move'], ['→', 'open'], ['←', 'back'], ['enter', 'details'], ['g', 'go to pane'], ['a', 'answer'], ['i', 'inbox'], ['f', 'filter'], ['?', 'help']]);
  return s;
}

// ───────── 3a · docked panel (swarm-dock.js vA), 46 columns of a 120 × h screen
const PW = 46, HOV = '#171a1f';
const ATT = st => st === 'wait' || st === 'blk' || st === 'fail';
const on = () => Math.floor(F / 2) % 2 === 0;
const gcol = st => ATT(st) ? blinkCol(th, th.st[st].t, F) : th.st[st].t; // eased (swarm-smooth.js)
const sl = segs => segs.reduce((a, g) => a + (g == null ? 0 : len(typeof g === 'string' ? g : g[0])), 0);
const RP = (s, xr, y, segs) => s.P(xr - sl(segs), y, segs);
const bar = (s, y, h) => { for (let j = 0; j < h; j++) s.put(0, y + j, '▌', th.key); };
// oState: done > failed > blocked > working > needs you > waiting > stale | idle
function oState(or) {
  const ags = or.threads.flatMap(t => t.agents), has = k => ags.some(a => a.st === k), r = ags.find(a => a.st === 'run'), q = nq(ags);
  const bg = (or.bg || []).length + ags.filter(a => a.st === 'bg').length;
  let st, l;
  if (has('fail')) [st, l] = ['fail', 'failed'];
  else if (has('blk')) [st, l] = ['blk', 'blocked'];
  else if (r || has('rev')) [st, l] = [r ? 'run' : 'rev', 'working'];
  else if (q) [st, l] = ['wait', 'needs you'];
  else if (bg) [st, l] = ['bg', or.bgAge ? 'waiting ' + ago(or.bgAge) : 'waiting'];
  else if (or.last > STALE_AFTER) [st, l] = ['stale', 'stale ' + ago(or.last)];
  else if (or.threads.length && or.threads.every(isDone)) [st, l] = ['done', 'done'];
  else [st, l] = ['q', 'idle'];
  return { st, l, q, bg, g: st === 'run' ? glyph(r) : st === 'bg' ? clock() : GL[st], working: !!r || has('rev'), ags, live: ags.filter(LIVE), act: or.threads.filter(t => !isDone(t)), done: or.threads.filter(isDone) };
}
const stCol = o => ATT(o.st) ? th.st[o.st].t : o.working ? th.st.run.t : o.st === 'bg' ? th.st.bg.t : th.dim;
function tState(t) { if (isDone(t)) return { st: 'done', g: '✓' }; const u = t.agents.filter(LIVE).sort(byUrg)[0]; return u ? { st: u.st, g: glyph(u) } : { st: 'q', g: '○' }; }
// glyph rows keep one space between glyphs (swarm-dock.js since the ghostty report)
const spaced = gs => gs.flatMap((g, i) => (i ? [' ', g] : [g]));
// 9e (swarm-tiles.js, the shipped docked panel): 2a's pods as tiles for the state, line 2 and the
// docked list; needs-you tiles blink to OFF; the agents of a listed thread stay bare glyphs
const OFF = ['#2a2e35', '#4d535c'];
const tile = (st, g) => { if (!ATT(st)) return [' ' + g + ' ', PB[st][1], 1, PB[st][0]]; const k = blinkK(F); return [' ' + g + ' ', mix(OFF[1], PB[st][1], k), 1, mix(OFF[0], PB[st][0], k)]; };
const bare = (st, g) => [g, th.st[st].t, ATT(st)];
const agSegs = (ags, max = 4) => { const live = ags.filter(LIVE).sort(byUrg), fin = ags.length - live.length; const o = spaced(live.slice(0, max).map(a => bare(a.st, glyph(a)))); if (fin) o.push([' +' + fin + '✓', th.faint]); return o; };
const thSegs = o => { const ts = o.act.map(t => { const x = tState(t); return tile(x.st, x.g); }); if (o.done.length) ts.push(tile('done', '✓' + (o.done.length > 1 ? o.done.length : ''))); return spaced(ts); };
const tItems = or => { const act = or.threads.filter(t => !isDone(t)), d = or.threads.length - act.length; return [...(or.bg || []).map(b => ({ bgt: b })), ...act, ...(d ? [{ fold: d }] : [])]; };
const right = o => [o.q ? ['?' + o.q, th.st.wait.t, 1] : null, o.q ? ' ' : null, [o.l, stCol(o)]];
function frame(H_, DOCK) {
  const s = new Scr(120, H_, th), all = DATA.flatMap(o => o.threads.flatMap(t => t.agents)), n = k => all.filter(a => a.st === k).length;
  for (let y = 0; y < H_; y++) s.put(PW, y, '│', th.line);
  s.P(1, 0, [['SWARM', th.dim, 1], ['  ' + DATA.length + ' orchestrators', th.faint]]);
  RP(s, 45, 0, [['?' + nq(all), th.st.wait.t, 1], ' ', ['!' + n('blk'), th.st.blk.t, 1], ' ', ['✗' + n('fail'), th.st.fail.t, 1]]);
  s.hl(0, H_ - 2, PW, th.line); s.put(PW, H_ - 2, '┤', th.line);
  const K = th.key, f = th.faint;
  s.P(1, H_ - 1, [['↑↓', K], [' move ', f], ['ret', K], [' open ', f], ['a', K], [' answer ', f], ['n', K], [' new ', f], ['pfx+spc', K], [' back', f]]);
  chat(s, H_, DOCK); return s;
}
function chat(s, H_, DOCK) {
  const X = PW + 2, w = 120 - X - 1, L = (y, segs) => s.P(X, y, segs.map(g => typeof g === 'string' ? g : [trunc(g[0], w), g[1], g[2]]));
  const or = DATA.find(o => o.name === DOCK);
  L(0, [[DOCK, th.hi, 1], ['  tab ' + or.tab + ' · ' + FL[or.fl].key + ' · claude code', th.faint]]);
  s.hl(PW + 1, 1, 120 - PW - 1, th.line); s.put(PW, 1, '├', th.line);
  L(3, [['> ', th.dim], ['Roll the new console image out on dev and tell me when it is live.', th.fg]]);
  L(5, [['● ', th.st.run.t], ['Pushed; the dev auto-deploy started.', th.fg]]);
  L(6, [['  └ Bash(Wait for the console image to roll out on dev) · running in background', th.faint]]);
  L(7, [['  └ Monitor(dev rollout events · argocd sync) · watching', th.faint]]);
  L(9, [['● ', th.st.run.t], ['Both watches run in the background; I will report when the rollout lands.', th.fg]]);
  rbox(s, X, H_ - 5, w + 1, 3, th.line);
  s.P(X + 2, H_ - 4, [['> ', th.dim], [on() ? '█' : ' ', th.fg]]);
  L(H_ - 2, [['  2 background tasks · esc to interrupt · ? for shortcuts', th.faint]]);
}
function newAgent(s, y) { s.P(2, y, [['+', th.key, 1], ' ', ['New agent', th.dim]]); RP(s, 45, y, [['n', th.faint]]); }
function panelS(H_, DOCK, HOVER) {
  const s = frame(H_, DOCK); let y = 2;
  for (const or of DATA) {
    const o = oState(or), dock = or.name === DOCK, hov = or.name === HOVER, items = dock ? tItems(or) : [], stale = o.st === 'stale';
    if (dock) { s.bg(0, y, PW, 2, th.sel); bar(s, y, 2 + items.length); } else if (hov) { s.bg(0, y, PW, 2, HOV); s.put(0, y, '›', th.dim); }
    const nameC = dock || hov ? th.hi : stale ? th.faint : o.st === 'done' ? th.dim : th.fg;
    s.P(2, y, [[String(or.tab), th.faint], ' ', tile(o.st, o.g), ' ', [trunc(or.name, 22), nameC, dock]]);
    RP(s, 45, y, right(o));
    const extra = o.bg && o.st === 'bg' ? ' · ' + o.bg + ' bg' : '';
    const cnt = o.act.length + ' active · ' + o.live.length + ' agents' + extra, segs = thSegs(o);
    if (items.length) s.P(8, y + 1, [[cnt, th.faint]]); // 9e: the docked list shows the tiles
    else s.P(8, y + 1, [...segs, [(segs.length ? '  ' : '') + cnt, th.faint]]);
    y += 2;
    items.forEach((t, i) => {
      s.put(3, y, i === items.length - 1 ? '└' : '├', th.line);
      if (t.fold) s.P(5, y, [tile('done', '✓'), ' ', [t.fold + ' done', th.faint]]);
      else if (t.bgt) { s.P(5, y, [tile('bg', clock(i + 1)), ' ', [trunc(t.bgt.desc, 22), th.fg]]); RP(s, 45, y, [[t.bgt.kind.toLowerCase() + ' ' + dur(NOW - tm(t.bgt.since)), th.dim]]); }
      else { const ts = tState(t); s.P(5, y, [tile(ts.st, ts.g), ' ', [trunc(t.name, 22), th.fg]]); RP(s, 45, y, agSegs(t.agents, 4)); }
      y++;
    });
    y++;
  }
  newAgent(s, y); return s;
}

export function renderStates(f = 0) {
  F = f; setFrame(Math.floor(f)); // f may be fractional: colours glide, spinners and the clock step
  const GH = 56;
  return {
    A: renderGraphS(160, GH, { orch: 'Deploy Watch', selT: 'bg:' }).rows(),
    B: renderGraphS(160, GH, { orch: n1.t.or.name, selT: 'p214', selA: 'n1' }).rows(),
    R: renderGraphS(160, GH, { orch: n1.t.or.name, selT: 'p214', selA: 'n1', f: 'running' }).rows(),
    P: panelS(GH, 'Deploy Watch', 'Metrics Export').rows(),
  };
}
export const STATES = { DATA, orchSt, oState };
