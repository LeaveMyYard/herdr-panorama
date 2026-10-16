// Minimal, Argo-style Swarm view: cards + solid box-drawing links + square pods.
// Reference copy for herdr-panorama (design turn 3: "lines and some animations", every card's edge animated).
import { TH, DATA, H } from './swarm-tui.js';
import { blinkCol } from './swarm-smooth.js';
let F = 0;
const { setFrame, Scr, IDX, TIDX, LIVE, byUrg, isDone, curIdx, roleL, glyph, lastPh, fit, trunc, wrap, tm, dur, NOW, FL, LBL, phaseGlyph, mark, markLen, len, urgent } = H;

const PB = { done: ['#2c6b4c', '#e3f2e9'], run: ['#1f625c', '#d8f2ee'], rev: ['#2e4f86', '#dfe8f8'], q: ['#2a2e35', '#9aa0aa'], wait: ['#f2a93b', '#1b1203'], blk: ['#f2a93b', '#1b1203'], fail: ['#d8363e', '#ffffff'], cxl: ['#24272c', '#6b717a'], next: ['#1c1f24', '#454b54'] };

function pod(s, x, y, st, g, sel) { const [b, f] = PB[st]; s.P(x, y, [[' ' + g + ' ', f, 1, b]]); if (sel) { s.put(x - 1, y, '[', s.th.hi, undefined, 1); s.put(x + 3, y, ']', s.th.hi, undefined, 1); } }
const apod = (s, x, y, a, sel) => pod(s, x, y, a.st, glyph(a), sel);
function rbox(s, x, y, w, h, col) { s.put(x, y, '╭' + '─'.repeat(w - 2) + '╮', col); for (let j = 1; j < h - 1; j++) { s.put(x, y + j, '│', col); s.put(x + w - 1, y + j, '│', col); } s.put(x, y + h - 1, '╰' + '─'.repeat(w - 2) + '╯', col); }
function tags(s, x, y, w, list) { let xx = x + w - 2; for (const t of list.filter(Boolean).reverse()) { xx -= len(t) + 2; s.P(xx, y, [[' ' + t + ' ', s.th.dim]]); xx -= 1; } }
function marksRight(s, xr, y, t) { const mk = mark(s.th, t); let w = mk.reduce((a, m) => a + len(m[0]) + 1, 0) - 1; let x = xr - w; mk.forEach(m => { x = s.P(x, y, [m]) + 1; }); return Math.max(0, w); }
// Box-drawing merge: U=1 D=2 L=4 R=8. Rounded corners; tees where a link meets a border.
const BX = { 3: '│', 12: '─', 10: '╭', 9: '╰', 6: '╮', 5: '╯', 11: '├', 7: '┤', 14: '┬', 13: '┴', 15: '┼', 1: '│', 2: '│', 4: '─', 8: '─' };
const XB = { '│': 3, '─': 12, '╭': 10, '╰': 9, '╮': 6, '╯': 5, '┌': 10, '└': 9, '┐': 6, '┘': 5, '├': 11, '┤': 7, '┬': 14, '┴': 13, '┼': 15 };
function jn(s, x, y, bits, col) {
  if (y < 0 || y >= s.h || x < 0 || x >= s.w) return;
  const c = s.c[y][x], had = XB[c.ch];
  if (c.ch === '┃') { if (bits === 4) c.ch = '┨'; else if (bits === 8) c.ch = '┠'; return; }
  if (had != null) c.ch = BX[had | bits]; else { c.ch = BX[bits]; c.fg = col; c.bg = null; c.b = 0; }
}
// fx = source box right border col, toX = target box left border col, tx = trunk col
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
function stepLine(s, x, y, t, i, w) {
  const th = s.th, p = t.ph[i], cur = p[1] === 'n' || p[1] === 'f', [g, gc] = phaseGlyph(th, t, i);
  const time = p[1] === 'x' ? '' : p[3] ? `${p[2]}–${p[3]}` : `since ${p[2]} · ${dur(NOW - tm(p[2]))}`;
  const xx = s.P(x, y, [[g, gc, 1], ' ', [fit(p[0], 13), cur ? th.hi : p[1] === 'x' ? th.faint : th.fg, cur], [fit(time, 20), cur ? th.fg : th.faint]]);
  if (p[4] && xx < x + w - 2) s.P(xx, y, [[trunc(p[4], x + w - xx), th.dim]]);
}
function hdr(s, o) {
  const th = s.th; s.bg(0, 0, s.w, 1, th.hdr);
  let x = s.P(1, 0, [['herdr', th.hi, 1], [' swarm', th.dim]]) + 4;
  for (const [k, l] of [['all', 'all'], ['running', 'running'], ['needs', 'needs me']]) { const on = (o.f || 'all') === k; x = s.P(x, 0, [[l, on ? th.hi : th.faint, on]]) + 3; }
  const segs = [['? 3', th.st.wait.t, 1], '  ', ['! 1', th.st.blk.t, 1], '  ', ['✗ 1', th.st.fail.t, 1], '    ', ['10:04', th.dim]];
  const L = segs.reduce((a, g) => a + len(typeof g === 'string' ? g : g[0]), 0); s.P(s.w - L - 1, 0, segs);
}
function legend(s, y) {
  const th = s.th, wide = s.w >= 120; let x = 1;
  const L = [['done', '✓', 'done'], ['run', '⠋', 'running'], ['rev', '»', 'review'], ['q', '○', 'queued'], ['wait', '?', 'question'], ['blk', '!', 'blocked'], ['fail', '✗', 'failed']];
  for (const [k, g, l] of L) { if (wide) { pod(s, x, y, k, g); x = s.P(x + 4, y, [[l, th.faint]]) + 3; } else x = s.P(x, y, [[g, th.st[k].t, 1], [' ' + ({ running: 'run', question: 'ask', blocked: 'block', failed: 'fail' }[l] || l), th.faint]]) + 2; }
}
function keys(s, list) { const th = s.th, y = s.h - 1; let x = 1; for (const [k, l] of list) x = s.P(x, y, [[k, th.fg, 1], [' ' + l, th.faint]]) + 3; }
function agentDetailLines(s, a, W) {
  const th = s.th, out = [];
  out.push([['now  ', th.faint], [trunc(a.act, W - 5), th.fg]]);
  if (a.qs) a.qs.forEach(q => wrap(q, W - 2).forEach((l, i) => out.push([[i ? '  ' : '? ', th.st.wait.t, 1], [l, th.st.wait.t]])));
  if (a.perm) out.push([['! ', th.st.blk.t, 1], ['wants ', th.dim], [a.perm, th.st.blk.t, 1]]);
  if (a.err) out.push([['✗ ', th.st.fail.t, 1], [trunc(a.err, W - 2), th.st.fail.t]]);
  if (a.msg && !a.qs) wrap('“' + a.msg + '”', W).slice(0, 2).forEach(l => out.push([[l, th.dim]]));
  return out;
}
function actions(s, x, y, a) {
  const th = s.th, can = a.st === 'wait' || a.st === 'blk';
  let xx = x; for (const [k, l, on] of [['g', 'open pane', 1], ['t', s.w >= 120 && x < 100 ? 'transcript graph' : 'transcript', 1], ['a', a.st === 'blk' ? 'allow / deny' : 'answer', can]]) xx = s.P(xx, y, [[k, on ? th.key : th.faint, 1], [' ' + l, on ? th.fg : th.faint]]) + 3;
}

// Orchestrator state from its agents: attention > working > idle > done.
// Animated card edge: blinking left bar when it needs you, a sweep along the bottom while agents work.
function edge(s, x, y, w, h, os, seed = 0) {
  if (!os || os.st === 'q' || os.st === 'done' || os.st === 'cxl') return;
  const th = s.th, c = th.st[os.st].t, att = os.st === 'wait' || os.st === 'blk' || os.st === 'fail', col = att ? blinkCol(th, c, F) : c;
  for (let j = 1; j < h - 1; j++) s.put(x, y + j, '┃', col);
  if (os.working) { const n = w - 2, p = (Math.floor(F) + seed * 7) % (n + 6) - 3; for (let i = 0; i < 4; i++) { const xx = p + i; if (xx >= 0 && xx < n) s.put(x + 1 + xx, y + h - 1, '━', th.st.run.t); } }
}
const agSt = a => ({ st: a.st, working: a.st === 'run' });
const thSt = t => stOf(t.agents, isDone(t));
const orchSt = or => stOf(or.threads.flatMap(t => t.agents), or.threads.every(isDone));
function stOf(ags, allDone) {
  const has = k => ags.some(a => a.st === k);
  if (allDone) return { st: 'done', g: '✓', l: 'all done' };
  for (const [k, g, l] of [['fail', '✗', 'failed'], ['blk', '!', 'blocked'], ['wait', '?', 'needs you']]) if (has(k)) {
    const r = ags.find(a => a.st === 'run'); return { st: k, g, l: l + (r ? ' · working' : ''), working: !!r };
  }
  const r = ags.find(a => a.st === 'run'); if (r) return { st: 'run', g: glyph(r), l: 'working', working: true };
  if (has('rev')) return { st: 'rev', g: '»', l: 'reviewing' };
  return { st: 'q', g: '○', l: 'idle' };
}

// ───────── 2a · Graph
function renderGraph(w, h, o) {
  const th = TH.dark, s = new Scr(w, h, th);
  hdr(s, o);
  if (w >= 120) {
    const OX = 2, OW = 28, TX = 36, TW = 50, AX = 93, AW = 65;
    let y = 3, selY = 0;
    for (const or of DATA) {
      const sel = or.name === o.orch;
      const ags = or.threads.flatMap(t => t.agents), os = orchSt(or), quiet = os.st === 'q' || os.st === 'done';
      rbox(s, OX, y, OW, 4, sel ? th.dim : th.line);
      edge(s, OX, y, OW, 4, os);
      const q = ags.reduce((a, g) => a + (g.st === 'wait' ? g.q : 0), 0), b = ags.some(g => g.st === 'blk'), f = ags.some(g => g.st === 'fail');
      pod(s, OX + 2, y + 1, os.st, os.g);
      s.P(OX + 6, y + 1, [[trunc(or.name, OW - 13), sel ? th.hi : quiet ? th.dim : th.fg, 1]]);
      const mk = [f && os.st !== 'fail' ? ['✗', th.st.fail.t, 1] : null, b && os.st !== 'blk' ? ['!', th.st.blk.t, 1] : null, q ? ['?' + q, th.st.wait.t, 1] : null].filter(Boolean);
      let mx = OX + OW - 2 - mk.reduce((a, m) => a + len(m[0]) + 1, -1); mk.forEach(m => { mx = s.P(mx, y + 1, [m]) + 1; });
      const nl = ags.filter(LIVE).length, na = or.threads.filter(t => !isDone(t)).length;
      const sl = trunc(os.l, OW - 8), rest = quiet ? '' : trunc(` · ${na}/${nl}`, OW - 8 - len(sl));
      s.P(OX + 6, y + 2, [[sl, quiet ? th.faint : th.st[os.st].t, !quiet], [rest, th.dim]]);
      tags(s, OX, y + 3, OW, [FL[or.fl].key]);
      if (sel) selY = y + 1;
      y += 5;
    }
    const or = DATA.find(d => d.name === o.orch);
    let ty = 3; const anchors = []; let selAnchor = 0;
    for (const t of or.threads.filter(t => !isDone(t))) {
      const sel = t.id === o.selT, hh = sel ? 6 + t.ph.length : 5, ci = curIdx(t), p = t.ph[ci], [g, gc] = phaseGlyph(th, t, ci);
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
    link(s, OX + OW - 1, selY, 32, TX, anchors);
    // agents of the selected thread
    const t = TIDX[o.selT], ags = t.agents.slice().sort(byUrg), live = ags.filter(LIVE), fin = ags.filter(a => !LIVE(a));
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
      rbox(s, AX, ay, AW, hh, th.dim); edge(s, AX, ay, AW, hh, agSt(a), a._n); apod(s, AX + 2, ay + 1, a, false);
      s.P(AX + 6, ay + 1, [[roleL(t, a.role), th.hi, 1], [' ' + a.id, th.faint], [' · ' + a.model, th.dim]]);
      const st = LBL[a.st] + (a.since ? ' ' + a.since : ''); s.P(AX + AW - 2 - len(st), ay + 1, [[st, th.st[a.st].t, 1]]);
      lines.forEach((l, i) => s.P(AX + 4, ay + 3 + i, l));
      actions(s, AX + 4, ay + hh - 2, a);
      tags(s, AX, ay + hh - 1, AW, [a.age, `${a.tools} calls`, t.ph[lastPh(a)][0]]);
      aAnch.push(ay + 1); ay += hh + 1;
    }
    if (fin.length) {
      rbox(s, AX, ay, AW, 3, th.line); let x = AX + 2; for (const a of fin.slice(0, 5)) { apod(s, x, ay + 1, a); x += 4; }
      const roles = {}; fin.forEach(a => { const r = roleL(t, a.role); roles[r] = (roles[r] || 0) + 1; });
      s.P(x + 1, ay + 1, [[`${fin.length} finished · ` + Object.entries(roles).map(([r, n]) => n > 1 ? `${r} ×${n}` : r).join(', '), th.faint]]);
      aAnch.push(ay + 1);
    }
    link(s, TX + TW - 1, selAnchor, 89, AX, aAnch);
    legend(s, h - 3);
    keys(s, [['↑↓', 'move'], ['→', 'open'], ['←', 'back'], ['enter', 'details'], ['g', 'go to pane'], ['a', 'answer'], ['i', 'inbox'], ['f', 'filter'], ['?', 'help']]);
  } else {
    let y = 2;
    for (const or of DATA) {
      const sel = or.name === o.orch, ags = or.threads.flatMap(t => t.agents);
      const os = orchSt(or), quiet = os.st === 'q' || os.st === 'done';
      if (!sel) { pod(s, 1, y, os.st, os.g); s.P(5, y, [[or.name, quiet ? th.dim : th.fg], ['  ' + os.l, quiet ? th.faint : th.st[os.st].t]]); const q = ags.reduce((a, g) => a + (g.st === 'wait' ? g.q : 0), 0); const mk = [ags.some(g => g.st === 'fail') ? ['✗', th.st.fail.t, 1] : null, ags.some(g => g.st === 'blk') ? ['!', th.st.blk.t, 1] : null, q ? ['?' + q, th.st.wait.t, 1] : null].filter(Boolean); let mx = w - 1 - mk.reduce((a, m) => a + len(m[0]) + 1, 0); mk.forEach(m => { mx = s.P(mx, y, [m]) + 1; }); y += 2; continue; }
      pod(s, 1, y, os.st, os.g); s.P(5, y, [[or.name, th.hi, 1], ['  ' + os.l, quiet ? th.faint : th.st[os.st].t], [`  tab ${or.tab} · ${FL[or.fl].key}`, th.faint]]);
      const top = y; y += 2; const anchors = [];
      for (const t of or.threads.filter(t => !isDone(t))) {
        const exp = t.id === o.selT, ci = curIdx(t), p = t.ph[ci], [g, gc] = phaseGlyph(th, t, ci);
        const a = IDX[o.selA], det = exp ? agentDetailLines(s, a, w - 10) : [];
        const hh = exp ? 4 + 1 + t.ph.length + 1 + 1 + det.length + 1 + 1 : 4;
        rbox(s, 3, y, w - 4, hh, exp ? th.dim : th.line); edge(s, 3, y, w - 4, hh, thSt(t), anchors.length + 1);
        const mw = marksRight(s, w - 3, y + 1, t);
        s.P(5, y + 1, [[g, gc, 1], ' ', [trunc(t.name, w - 13 - mw), exp ? th.hi : th.fg, 1]]);
        podsRow(s, 7, y + 2, t, o, 6);
        const sub = p[0] + (p[2] ? ' · ' + dur(NOW - tm(p[2])) : '') + (t.pct != null ? ` · ${t.pct}%` : '');
        s.P(w - 3 - len(sub), y + 2, [[sub, th.dim]]);
        if (exp) {
          let r = y + 4; t.ph.forEach((_, i) => stepLine(s, 7, r++, t, i, w - 12)); r++;
          apod(s, 7, r, a); s.P(11, r, [[roleL(t, a.role), th.hi, 1], [` ${a.id} · ${a.model} · ${a.age} · ${a.tools} calls`, th.dim]]); r++;
          det.forEach(l => s.P(7, r++, l)); r++;
          actions(s, 7, r, a);
        }
        anchors.push(y + 1); y += hh;
      }
      const dn = or.threads.filter(isDone);
      if (dn.length) { s.P(5, y, [['✓', th.st.done.t, 1], [` ${dn.length} complete · ` + trunc(dn.map(t => t.name).join(', '), w - 20), th.faint]]); anchors.push(y); y++; }
      const last = anchors[anchors.length - 1];
      for (let yy = top + 1; yy <= last; yy++) jn(s, 1, yy, 1 | (yy < last ? 2 : 0) | (anchors.includes(yy) ? 8 : 0), th.line);
      anchors.forEach(ay => { jn(s, 2, ay, 12, th.line); if (s.c[ay][3].ch === '│' || s.c[ay][3].ch === '┃') jn(s, 3, ay, 4, th.line); });
      y += 1;
    }
    legend(s, h - 3);
    keys(s, [['↑↓', 'move'], ['→', 'open'], ['g', 'pane'], ['a', 'answer'], ['f', 'filter'], ['?', 'help']]);
  }
  return s.rows();
}

// ───────── 2b · Checklist (squares are workflow steps)
// (2b is not implemented by herdr-panorama; its renderer lives in the Claude Design
// project and is omitted from this reference copy.)

export function renderMin(f = 0) {
  F = f; setFrame(Math.floor(f));
  return {
    G1: renderGraph(160, 45, { orch: 'Payments UI', selT: 't595', selA: 'a2' }),
    G2: renderGraph(70, 45, { orch: 'herdr core', selT: 'w1', selA: 'i3' }),
  };
}

// The interactive prototype (Swarm View 2a.dc.html) steps f every 250 ms.
export function renderGraphAt(w, h, o, f = 0) { F = f; setFrame(Math.floor(f)); return renderGraph(w, h, o); }
