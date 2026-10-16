// Turn 7: wider cards on big screens. 2a with turn 5's depth columns (5d), the cards sized by a tier of the
// terminal width instead of fixed 28/50/44 columns. Columns still keep their width and align left (the
// operator: a stretched card makes its sweep run too long); a tier only widens every card a little, capped,
// so the agent's type, id and full model name fit, and wide screens show more depth columns.
//   < 220 columns   orchestrators 28 · threads 50 (36..50 below 160) · agents 44   (today)
//   220..299        orchestrators 32 · threads 56 · agents 56
//   >= 300          orchestrators 36 · threads 64 · agents 60
// The agent title (type, id, model) is cut with "…" before the right-aligned state, never into it.
// The working sweep along a card's bottom border takes at most 12 s per lap whatever the card's width (operator:
// wider cards must not animate slower); the connector lines are not animated.
// Agents carry their real type (general-purpose, Explore, ...), a 7-hex id and the model with its version.
import { TH, DATA, H } from './swarm-tui.js';
const { setFrame, Scr, LIVE, byUrg, isDone, curIdx, glyph, trunc, fit, len, GL, LBL, dur, tm, NOW } = H;
let F = 0;
const LAP = 48; // frames of 250 ms: 12 s, a 44-column card's lap (42 border cells + 6)
const th = TH.dark;
const PB = { done: ['#2c6b4c', '#e3f2e9'], run: ['#1f625c', '#d8f2ee'], rev: ['#2e4f86', '#dfe8f8'], q: ['#2a2e35', '#9aa0aa'], wait: ['#f2a93b', '#1b1203'], blk: ['#f2a93b', '#1b1203'], fail: ['#d8363e', '#ffffff'], cxl: ['#24272c', '#6b717a'] };
const ATT = st => st === 'wait' || st === 'blk' || st === 'fail';

// ── dataset: one thread whose agents nest five levels deep, with an eight-way fan-out
let N = 100;
const MODEL = { opus: 'opus 4.5', sonnet: 'sonnet 4.5', haiku: 'haiku 4.5' };
const TYPE = { lead: 'general-purpose', explorer: 'Explore', researcher: 'general-purpose', checker: 'code-reviewer', probe: 'Explore', fixer: 'general-purpose', batch: 'general-purpose', shard: 'Explore', reviewer: 'pr-review-toolkit:code-reviewer' };
const HEX = id => { let h = 2166136261; for (const c of id) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0; return h.toString(16).padStart(8, '0').slice(0, 7); }; // a stable 7-hex id
const ag = (id, role, st, act, kids = [], x = {}) => ({ id, hex: HEX(id), role: TYPE[role] || role, st, act, kids, age: x.age || '12m', tools: x.tools ?? 20, q: x.q || 0, _n: N++, ...x, model: MODEL[x.model || 'sonnet'] });
const SHARDS = Array.from({ length: 8 }, (_, i) => ag('s' + (i + 1), 'shard', ['run', 'run', 'done', 'done', 'done', 'fail', 'q', 'q'][i],
  ['indexing loaders/a…f', 'indexing loaders/g…m', 'indexed 214 files', 'indexed 98 files', 'indexed 133 files', 'parse error in loaders/x.rs', 'queued', 'queued'][i], [], { model: 'haiku', age: ['4m', '4m', '9m', '9m', '8m', '2m', '0m', '0m'][i], tools: [12, 9, 31, 22, 27, 6, 0, 0][i] }));
const TREE = [
  ag('l1', 'lead', 'run', 'splitting PluginLoader::load into stages', [
    ag('e1', 'explorer', 'done', 'mapped plugin call sites', [], { model: 'haiku', age: '41m', tools: 18 }),
    ag('e2', 'explorer', 'done', 'read docs/plugins.md', [], { model: 'haiku', age: '40m', tools: 7 }),
    ag('r1', 'researcher', 'run', 'tracing the manifest cache', [
      ag('e3', 'explorer', 'done', 'listed cache readers', [], { model: 'haiku', age: '20m', tools: 9 }),
      ag('r2', 'researcher', 'run', 'checking cache invalidation on reload', [
        ag('c1', 'checker', 'wait', 'paused · asks about the lockfile', [
          ag('p1', 'probe', 'run', 'running cargo test cache::', [], { model: 'haiku', age: '2m', tools: 4 }),
          ag('p2', 'probe', 'done', 'reproduced the stale entry', [], { model: 'haiku', age: '5m', tools: 6 }),
        ], { q: 1, qs: ['Bump Cargo.lock for the notify crate, or pin 6.1?'], age: '7m', tools: 11 }),
      ], { model: 'opus', age: '16m', tools: 38 }),
      ag('f1', 'fixer', 'fail', 'stopped: tests/cache_test.rs fails', [], { err: 'cargo test exited 101 · 2 failing', age: '9m', tools: 14 }),
    ], { model: 'opus', age: '24m', tools: 52 }),
    ag('b1', 'batch', 'run', 'indexing 8 shards', SHARDS, { age: '10m', tools: 19 }),
  ], { model: 'opus', age: '44m', tools: 97 }),
  ag('v1', 'reviewer', 'rev', 'diffing stage 1 against the plan', [], { model: 'opus', age: '6m', tools: 21 }),
];
const THREAD = { id: 'deep', name: 'Refactor: plugin loader', ph: [['explore', 'd', '09:20', '09:40'], ['build', 'n', '09:41'], ['verify', 'x'], ['merge', 'x']] };
const DOCK = 'Core Library';
const SEL = { a: 'c1', b: 'r1', c: 'r1' }; // the selected agent per direction

function walk(list, depth = 1, parent = null, out = []) { for (const a of list) { a.depth = depth; a.parent = parent; out.push(a); walk(a.kids, depth + 1, a, out); } return out; }
const ALL = walk(TREE);
const BY = Object.fromEntries(ALL.map(a => [a.id, a]));
const desc = a => a.kids.reduce((n, k) => n + 1 + desc(k), 0);
const path = id => { const p = []; for (let a = BY[id]; a; a = a.parent) p.unshift(a); return p; };

// ── 2a pieces (as in swarm-min.js)
function pod(s, x, y, st, g, sel) { const [b, f] = PB[st]; s.P(x, y, [[' ' + g + ' ', f, 1, b]]); if (sel) { s.put(x - 1, y, '[', th.hi, undefined, 1); s.put(x + 3, y, ']', th.hi, undefined, 1); } }
const apod = (s, x, y, a, sel) => pod(s, x, y, a.st, glyph(a), sel);
function rbox(s, x, y, w, h, col) { s.put(x, y, '╭' + '─'.repeat(w - 2) + '╮', col); for (let j = 1; j < h - 1; j++) { s.put(x, y + j, '│', col); s.put(x + w - 1, y + j, '│', col); } s.put(x, y + h - 1, '╰' + '─'.repeat(w - 2) + '╯', col); }
function tags(s, x, y, w, list) { let xx = x + w - 2; for (const t of list.filter(Boolean).reverse()) { xx -= len(t) + 2; s.P(xx, y, [[' ' + t + ' ', th.dim]]); xx -= 1; } }
const BX = { 3: '│', 12: '─', 10: '╭', 9: '╰', 6: '╮', 5: '╯', 11: '├', 7: '┤', 14: '┬', 13: '┴', 15: '┼', 1: '│', 2: '│', 4: '─', 8: '─' };
const XB = { '│': 3, '─': 12, '╭': 10, '╰': 9, '╮': 6, '╯': 5, '├': 11, '┤': 7, '┬': 14, '┴': 13, '┼': 15 };
function jn(s, x, y, bits, col = th.line) {
  if (y < 0 || y >= s.h || x < 0 || x >= s.w) return;
  const c = s.c[y][x], had = XB[c.ch];
  if (c.ch === '┃') { if (bits === 4) c.ch = '┨'; else if (bits === 8) c.ch = '┠'; return; }
  if (had != null) c.ch = BX[had | bits]; else { c.ch = BX[bits]; c.fg = col; c.bg = null; c.b = 0; }
}
function link(s, fx, fy, tx, toX, ys) {
  jn(s, fx, fy, 8); for (let x = fx + 1; x < tx; x++) jn(s, x, fy, 12);
  const all = [fy, ...ys], y0 = Math.min(...all), y1 = Math.max(...all);
  for (let y = y0; y <= y1; y++) jn(s, tx, y, (y > y0 ? 1 : 0) | (y < y1 ? 2 : 0) | (y === fy ? 4 : 0) | (ys.includes(y) ? 8 : 0));
  for (const y of ys) { for (let x = tx + 1; x < toX; x++) jn(s, x, y, 12); jn(s, toX, y, 4); }
}
function edge(s, x, y, w, h, os, seed = 0) {
  if (!os || os.st === 'q' || os.st === 'done' || os.st === 'cxl') return;
  const c = th.st[os.st].t, on = !ATT(os.st) || Math.floor(F / 2) % 2 === 0;
  for (let j = 1; j < h - 1; j++) s.put(x, y + j, '┃', on ? c : th.faint);
  // the sweep takes at most LAP frames per lap however wide the card: up to a 44-column card it moves one cell
  // a frame (as before); on wider cards it moves further per frame, so a lap never takes longer than 12 s
  if (os.working) { const n = w - 2, L = Math.min(n + 6, LAP), p = Math.floor(((F + seed * 7) % L) * (n + 6) / L) - 3; for (let i = 0; i < 4; i++) { const xx = p + i; if (xx >= 0 && xx < n) s.put(x + 1 + xx, y + h - 1, '━', th.st.run.t); } }
}
const agSt = a => ({ st: a.st, working: a.st === 'run' });
function stOf(ags) {
  const has = k => ags.some(a => a.st === k), r = ags.find(a => a.st === 'run');
  for (const [k, g, l] of [['fail', '✗', 'failed'], ['blk', '!', 'blocked'], ['wait', '?', 'needs you']]) if (has(k)) return { st: k, g, l: l + (r ? ' · working' : ''), working: !!r };
  if (r) return { st: 'run', g: glyph(r), l: 'working', working: true };
  return { st: 'q', g: '○', l: 'idle' };
}
function hdr(s) {
  s.bg(0, 0, s.w, 1, th.hdr);
  let x = s.P(1, 0, [['herdr', th.hi, 1], [' panorama', th.dim]]) + 4;
  for (const [k, l] of [['all', 'all'], ['running', 'running'], ['needs', 'needs me']]) x = s.P(x, 0, [[l, k === 'all' ? th.hi : th.faint, k === 'all']]) + 3;
  const segs = [['? 4', th.st.wait.t, 1], '  ', ['! 1', th.st.blk.t, 1], '  ', ['✗ 3', th.st.fail.t, 1], '    ', ['10:04', th.dim]];
  s.P(s.w - segs.reduce((a, g) => a + len(typeof g === 'string' ? g : g[0]), 0) - 1, 0, segs);
}
function legend(s, y) { let x = 1; for (const [k, g, l] of [['done', '✓', 'done'], ['run', '⠋', 'running'], ['rev', '»', 'review'], ['q', '○', 'queued'], ['wait', '?', 'question'], ['blk', '!', 'blocked'], ['fail', '✗', 'failed']]) { pod(s, x, y, k, g); x = s.P(x + 4, y, [[l, th.faint]]) + 3; } }
function keys(s, list) { const y = s.h - 1; let x = 1; for (const [k, l] of list) x = s.P(x, y, [[k, th.fg, 1], [' ' + l, th.faint]]) + 3; }

// the orchestrator and thread columns: 2a's, with the deep thread selected
let OX = 2, OW = 28, TX = 36, TW = 50, AX = 93;
// the width tier: every card a little wider on a big screen, capped (no stretching)
function tier(w) {
  if (w >= 300) return { OW: 36, TW: 64, DW: 60 };
  if (w >= 220) return { OW: 32, TW: 56, DW: 56 };
  return { OW: 28, TW: Math.max(36, Math.min(50, Math.floor((w - 36 - 7 - 2) * 50 / 115))), DW: 44 };
}
function orchColumn(s) {
  let y = 3, selY = 0;
  for (const or of DATA) {
    const sel = or.name === DOCK, ags = or.threads.flatMap(t => t.agents), os = sel ? stOf(ALL) : stOf(ags), quiet = os.st === 'q';
    rbox(s, OX, y, OW, 4, sel ? th.dim : th.line); edge(s, OX, y, OW, 4, os);
    pod(s, OX + 2, y + 1, os.st, os.g);
    s.P(OX + 6, y + 1, [[trunc(or.name, OW - 13), sel ? th.hi : th.fg, 1]]);
    s.P(OX + 6, y + 2, [[trunc(os.l, OW - 8), quiet ? th.faint : th.st[os.st].t, !quiet]]);
    tags(s, OX, y + 3, OW, [or.fl === 'co' ? 'coordinator' : or.fl === 'sp' ? 'superpowers' : 'pr-review']);
    if (sel) selY = y + 1; y += 5;
  }
  return selY;
}
function threadColumn(s) {
  const t = THREAD, hh = 6 + t.ph.length, ci = curIdx(t);
  rbox(s, TX, 3, TW, hh, th.dim); edge(s, TX, 3, TW, hh, stOf(ALL), 1);
  s.P(TX + 2, 4, [[glyph(BY.l1), th.st.run.t, 1], ' ', [t.name, th.hi, 1]]);
  s.P(TX + 4, 5, [['build · 23m · ' + ALL.length + ' agents, 5 levels', th.dim]]);
  let x = TX + 4; for (const a of TREE) { apod(s, x, 6, a, a.id === 'l1'); x += 4; }
  s.P(x, 6, [['+' + (ALL.length - TREE.length) + ' nested', th.faint]]);
  t.ph.forEach((p, i) => { const cur = i === ci, d = p[1] === 'd'; s.P(TX + 4, 8 + i, [[d ? '✓' : cur ? glyph(BY.l1) : '·', d ? th.st.done.t : cur ? th.st.run.t : th.faint, 1], ' ', [fit(p[0], 13), cur ? th.hi : d ? th.fg : th.faint, cur], [p[2] ? (p[3] ? `${p[2]}–${p[3]}` : `since ${p[2]} · 23m`) : '', cur ? th.fg : th.faint]]); });
  tags(s, TX, 3 + hh - 1, TW, ['44m', 'wt/loader']);
  let y = 3 + hh + 1; const anchors = [4];
  for (const [g, st, n] of [['⠧', 'run', 'Worker: pane resize events'], ['⠼', 'run', 'Worker: config hot-reload']]) {
    rbox(s, TX, y, TW, 5, th.line); edge(s, TX, y, TW, 5, { st, working: true }, anchors.length + 1);
    s.P(TX + 2, y + 1, [[g, th.st[st].t, 1], ' ', [n, th.fg, 1]]); s.P(TX + 4, y + 2, [['build · 22m', th.dim]]);
    pod(s, TX + 4, y + 3, 'run', g); pod(s, TX + 8, y + 3, 'done', '✓'); tags(s, TX, y + 4, TW, ['22m']);
    anchors.push(y + 1); y += 6;
  }
  return anchors;
}

// ── 5d · one column per depth level, scrolled sideways (the operator's direction)
// orchestrators | threads | depth 1 | depth 2 | ... as deep as the tree goes. Every agent of the selected
// thread sits in its depth's column, next to the agent that spawned it, joined by 2a's connector lines.
// The view shows the focused column with the one before it (its parents) and, if it fits, the one after
// it (its children); the columns that do not fit scroll off left or right, named on the line above.
// Columns keep their width and align left: a stretched column made the sweep along its bottom too slow.
let DW = 44; const DMIN = 36, GAP = 7; // a depth column's width (by tier); the last one shown may shrink to DMIN
const SHOWN = a => LIVE(a) || a.st === 'fail' || a.kids.some(SHOWN); // finished subtrees fold into "n finished"
function detail(a, W) { // 2a's agentDetailLines
  const out = [[['now  ', th.faint], [trunc(a.act, W - 5), th.fg]]];
  if (a.qs) a.qs.forEach(q => H.wrap(q, W - 2).forEach((l, i) => out.push([[i ? '  ' : '? ', th.st.wait.t, 1], [l, th.st.wait.t]])));
  if (a.err) out.push([['✗ ', th.st.fail.t, 1], [trunc(a.err, W - 2), th.st.fail.t]]);
  return out;
}
function acts(s, x, y, a) { // 2a's actions
  const can = a.st === 'wait' || a.st === 'blk';
  let xx = x; for (const [k, l, on] of [['g', 'open pane', 1], ['t', 'transcript', 1], ['a', a.st === 'blk' ? 'allow / deny' : 'answer', can]]) xx = s.P(xx, y, [[k, on ? th.key : th.faint, 1], [' ' + l, on ? th.fg : th.faint]]) + 3;
}
function v5d(w, h, focus) {
  const tw = tier(w); OW = tw.OW; TW = tw.TW; DW = tw.DW; TX = OX + OW + 6; AX = TX + TW + 7;
  const s = new Scr(w, h, th); hdr(s); legend(s, h - 3);
  const sel = focus === 'thread' ? null : focus, selPath = new Set(sel ? path(sel).map(a => a.id) : []);
  const D = Math.max(...ALL.filter(SHOWN).map(a => a.depth));
  // the columns and the window onto them
  const cols = [{ k: 'orch', w: OW }, { k: 'thread', w: TW }, ...Array.from({ length: D }, (_, i) => ({ k: 'd', d: i + 1, w: DW }))];
  const gapB = i => (i === 1 ? 6 : GAP);
  // the last column needs only DMIN; the window: the focus, its parents' column and, if it fits, its children's;
  // then as many more as fit, right first, then left
  const span = (lo, hi) => { let n = 0; for (let i = lo; i <= hi; i++) n += (i === hi && i >= 2 ? DMIN : cols[i].w) + (i > lo ? gapB(i) : 0); return n; };
  const f = sel ? 1 + BY[sel].depth : 1, fit6 = w - 6;
  let lo = f <= 1 ? 0 : f - 1, hi = Math.min(cols.length - 1, Math.max(f + 1, 2));
  while (hi > f && span(lo, hi) > fit6) hi--;
  while (lo < f && span(lo, hi) > fit6) lo++;
  while (hi + 1 < cols.length && span(lo, hi + 1) <= fit6) hi++;
  while (lo > 0 && span(lo - 1, hi) <= fit6) lo--;
  const x0 = lo === 0 ? 2 : 4, avail = w - 2 - x0; // scrolled: 2 more columns for the lines in from the left
  if (hi >= 2) cols[hi].w = Math.min(DW, avail - (span(lo, hi) - DMIN)); // columns keep their width (the last one
  // shrinks to fit, down to DMIN); the space left on the right stays empty (operator, turn 5)
  let x = x0; for (let i = lo; i <= hi; i++) { if (i > lo) x += gapB(i); cols[i].x = x; x += cols[i].w; }
  const colOf = d => cols[1 + d], vis = d => 1 + d >= lo && 1 + d <= hi;
  // the line above the columns: what scrolled off, left and right
  if (lo > 0) {
    const names = [['Core Library'], [THREAD.name], ...[...selPath].map(id => [BY[id].role + ' ' + BY[id].hex])].slice(0, lo);
    s.P(2, 2, [['‹ ', th.dim, 1], ...names.flatMap((n, i) => [[n[0], i === lo - 1 ? th.fg : th.dim], [i < lo - 1 ? ' › ' : '', th.faint]]), [`   ${lo} column${lo > 1 ? 's' : ''} left`, th.faint]]);
  }
  if (hi < cols.length - 1) { const n = cols.length - 1 - hi, t = `depth ${hi}${n > 1 ? '–' + (cols.length - 2) : ''}  ${n} column${n > 1 ? 's' : ''} right ›`; s.P(w - 2 - len(t), 2, [[t.slice(0, -1), th.faint], ['›', th.dim, 1]]); }
  // orchestrators and threads (2a, unchanged) when they are in view
  let tAnchor = null;
  if (lo === 0) { const sy = orchColumn(s), an = threadColumn(s); link(s, OX + OW - 1, sy, OX + OW + 2, TX, an); tAnchor = 4; }
  // the agent tree, laid out on a tall canvas. Each column stacks its cards; an agent's children start
  // level with it (or below the cards already in their column), and an agent with children starts below
  // the previous group's last child, so two groups' connector lines never share rows.
  const T = new Scr(w, 200, th), nodes = [], groups = [], cur = {}, trunkEnd = {};
  const cardH = a => (a.id === sel ? 6 + detail(a, colOf(a.depth).w - 6).length : 3);
  function place(list, d, y0, parent) {
    const lv = list.filter(SHOWN), fin = list.filter(a => !SHOWN(a)); const ys = [];
    const at = (n, kids) => { n.y = Math.max(cur[d] ?? 0, d > 1 || parent ? y0 : 0, kids ? (trunkEnd[d + 1] ?? -2) + 1 : -1); cur[d] = n.y + n.h + 1; ys.push(n.y + 1); nodes.push(n); };
    for (const a of lv) {
      const n = { a, d, h: cardH(a) }, kids = a.kids.length > 0; at(n, kids);
      if (kids) place(a.kids, d + 1, n.y, n);
    }
    if (fin.length) at({ fold: fin, d, h: 3 }, false);
    trunkEnd[d] = Math.max(trunkEnd[d] ?? -1, ys[ys.length - 1]);
    groups.push({ parent, d, ys });
  }
  place(TREE, 1, 0, null);
  const end = Math.max(...Object.values(cur));
  for (const n of nodes) {
    if (!vis(n.d)) continue;
    const c = colOf(n.d), cw = c.w, cx = c.x, y = n.y;
    if (n.fold) {
      rbox(T, cx, y, cw, 3, th.line); let px = cx + 2; for (const a of n.fold.slice(0, 5)) { apod(T, px, y + 1, a); px += 4; }
      const roles = {}; n.fold.forEach(a => { roles[a.role] = (roles[a.role] || 0) + 1; });
      T.P(px + 1, y + 1, [[trunc(`${n.fold.length} finished · ` + Object.entries(roles).map(([r, k]) => k > 1 ? `${r} ×${k}` : r).join(', '), cx + cw - px - 5), th.faint]]); T.put(cx + cw - 3, y + 1, '▸', th.faint);
      continue;
    }
    const a = n.a, isSel = a.id === sel, onPath = selPath.has(a.id), kidsOff = a.kids.some(SHOWN) && !vis(n.d + 1);
    if (!isSel) {
      rbox(T, cx, y, cw, 3, onPath ? th.dim : th.line); edge(T, cx, y, cw, 3, agSt(a), a._n); apod(T, cx + 2, y + 1, a);
      const right = kidsOff ? `› ${a.kids.length}` : '';
      const x2 = T.P(cx + 6, y + 1, fitSegs([[a.role, onPath ? th.hi : th.fg, 1], [' ' + a.hex, th.faint], [' · ' + a.model, th.dim]], cx + cw - 2 - (right ? len(right) + 1 : 0) - (cx + 6)));
      T.P(x2 + 3, y + 1, [[trunc(a.act, Math.max(0, cx + cw - 3 - x2 - 3 - (right ? len(right) + 1 : 0))), th.faint]]);
      if (right) T.P(cx + cw - 2 - len(right), y + 1, [[right, th.dim, 1]]);
      tags(T, cx, y + 2, cw, [a.age, a.tools ? `${a.tools} calls` : null]);
      continue;
    }
    const lines = detail(a, cw - 6);
    rbox(T, cx, y, cw, n.h, th.hi); edge(T, cx, y, cw, n.h, agSt(a), a._n); apod(T, cx + 2, y + 1, a);
    const st = LBL[a.st]; T.P(cx + cw - 2 - len(st), y + 1, [[st, th.st[a.st].t, 1]]);
    T.P(cx + 6, y + 1, fitSegs([[a.role, th.hi, 1], [' ' + a.hex, th.faint], [' · ' + a.model, th.dim]], cx + cw - 2 - len(st) - 1 - (cx + 6)));
    lines.forEach((l, i) => T.P(cx + 4, y + 3 + i, l));
    acts(T, cx + 4, y + n.h - 2, a);
    tags(T, cx, y + n.h - 1, cw, [a.age, a.tools ? `${a.tools} calls` : null, 'build']);
  }
  // lines: each group of children to the agent that spawned it (from the left edge when that one scrolled off)
  for (const g of groups) {
    if (!vis(g.d)) continue;
    const c = colOf(g.d), tx = c.x - 4;
    if (g.parent) { const pv = vis(g.d - 1), pc = colOf(g.d - 1); link(T, pv ? pc.x + pc.w - 1 : 0, g.parent.y + 1, pv ? tx : 1, c.x, g.ys); }
    else if (tAnchor == null) link(T, c.x - GAP, g.ys[0], c.x - 4 < 1 ? 1 : tx, c.x, g.ys);
  }
  // blit the visible rows of the tree; the selection decides the vertical offset (it is at the top here)
  const top = 3, bottom = h - 5, X0 = lo === 0 ? AX - 4 : 0;
  for (let y = 0; y <= bottom - top; y++) for (let xx = X0; xx < w; xx++) { const c = T.c[y][xx]; if (c.ch !== ' ' || c.bg) s.c[top + y][xx] = { ...c }; }
  if (tAnchor != null) { const d1 = groups.find(g => !g.parent); link(s, TX + TW - 1, tAnchor, TX + TW + 3, colOf(1).x, d1.ys.map(v => v + top).filter(v => v <= bottom)); }
  if (end > bottom - top + 1) { const c = colOf(lo <= 2 ? 1 : lo - 1); s.P(c.x + 2, h - 4, [['↓ ', th.faint], ['more below', th.faint]]); }
  const sideways = lo > 0 || hi < cols.length - 1;
  keys(s, [['↑↓', 'move'], ['→', 'open'], ['←', 'back'], ...(sideways ? [['shift+wheel', 'scroll']] : []), ['enter', 'details'], ['g', 'go to pane'], ['a', 'answer'], ['i', 'inbox'], ['f', 'filter'], ['?', 'help']]);
  return s;
}


// the title cut to its room with "…" (the app's fit_segs): never into what is right-aligned after it
function fitSegs(segs, room) {
  const full = segs.map(g => g[0]).join('');
  if (len(full) <= room) return segs;
  const cut = Array.from(trunc(full, Math.max(1, room))); let pos = 0; const out = [];
  for (const g of segs) { const n = len(g[0]), piece = cut.slice(pos, pos + n).join(''); pos += n; if (piece) out.push([piece, ...g.slice(1)]); }
  return out;
}

// four working cards of 28, 44, 56 and 60 columns, sweeping together: the same lap time
function sweeps() {
  const s = new Scr(206, 4, th); let x = 1;
  for (const w of [28, 44, 56, 60]) { rbox(s, x, 0, w, 3, th.line); edge(s, x, 0, w, 3, { st: 'run', working: true }, 0); s.P(x + 2, 1, [[`${w} columns`, th.dim]]); x += w + 3; }
  return s;
}

export function renderWide(f = 0) {
  F = f; setFrame(f);
  return { W160: v5d(160, 45, 'thread').rows(), W240: v5d(240, 45, 'thread').rows(), W320: v5d(320, 45, 'thread').rows(),
    R160: v5d(160, 45, 'r1').rows(), R320: v5d(320, 45, 'r1').rows(), S: sweeps().rows() };
}

// for the herdr-panorama golden test (tests/fixtures/capture_design.py)
export { TREE, THREAD };
