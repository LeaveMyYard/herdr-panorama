// Cell-grid renderer + sample data for the Swarm view mockups.
// Reference copy for herdr-panorama: themes, sample data and the helpers that 2a and 3b use.
// The turn-1 renderers (A pod grid, B lanes, C tree, D attention) are omitted;
// they live in the Claude Design project.
const arr = s => Array.from(String(s));
const len = s => arr(s).length;
const fit = (s, n) => { const a = arr(s); if (n <= 0) return ''; if (a.length > n) return a.slice(0, n - 1).join('') + '…'; return a.join('') + ' '.repeat(n - a.length); };
const trunc = (s, n) => { const a = arr(s); return a.length > n ? a.slice(0, Math.max(0, n - 1)).join('') + '…' : String(s); };
const wrap = (t, w) => { const out = []; let cur = ''; for (const wd of String(t).split(' ')) { if (!cur) cur = wd; else if (len(cur) + 1 + len(wd) <= w) cur += ' ' + wd; else { out.push(cur); cur = wd; } } if (cur) out.push(cur); return out.map(l => trunc(l, w)); };
const NOW = 10 * 60 + 4;
const tm = s => { const [h, m] = s.split(':').map(Number); return h * 60 + m; };
const dur = m => m >= 60 ? `${Math.floor(m / 60)}h${String(m % 60).padStart(2, '0')}m` : `${m}m`;

export const TH = {
  dark: { bg: '#101215', fg: '#c3c7cd', hi: '#eceef1', dim: '#80868f', faint: '#4d535c', line: '#2c313a', sel: '#1e2937', nowBg: '#191e25', hdr: '#171a1f', key: '#e2cf9c', chipBg: '#c3c7cd', chipFg: '#101215',
    st: { q: { t: '#8d939d', pf: '#a3a9b3', pb: '#252930' }, run: { t: '#68c2b5', pf: '#86dccf', pb: '#123430' }, rev: { t: '#8eaee6', pf: '#aac4f2', pb: '#1a2944' }, wait: { t: '#ffb547', pf: '#1b1203', pb: '#f2a93b' }, blk: { t: '#ffb547', pf: '#1b1203', pb: '#f2a93b' }, done: { t: '#6c9673', pf: '#7aa481', pb: '#172219' }, fail: { t: '#ff6b6b', pf: '#ffffff', pb: '#d8363e' }, cxl: { t: '#646a73', pf: '#70767f', pb: '#1b1e23' } } },
  light: { bg: '#f6f5f1', fg: '#2b2e33', hi: '#0f1114', dim: '#646973', faint: '#a0a4aa', line: '#d5d3cc', sel: '#dbe5f1', nowBg: '#ebeae3', hdr: '#e8e7e1', key: '#7a5a12', chipBg: '#2b2e33', chipFg: '#f6f5f1',
    st: { q: { t: '#666b74', pf: '#555a63', pb: '#e1e0da' }, run: { t: '#1d7d70', pf: '#135a50', pb: '#cde8e3' }, rev: { t: '#3360ae', pf: '#22468a', pb: '#d6e0f3' }, wait: { t: '#a86300', pf: '#1b1203', pb: '#f0a12e' }, blk: { t: '#a86300', pf: '#1b1203', pb: '#f0a12e' }, done: { t: '#4d7d56', pf: '#3d6b46', pb: '#dce8de' }, fail: { t: '#c4262c', pf: '#ffffff', pb: '#d3313a' }, cxl: { t: '#9a9ea5', pf: '#888c93', pb: '#eae9e4' } } },
};

const GL = { q: '○', run: '⠋', rev: '»', wait: '?', blk: '!', done: '✓', fail: '✗', cxl: '-' };
const LBL = { q: 'queued', run: 'running', rev: 'reviewing', wait: 'needs answer', blk: 'blocked', done: 'done', fail: 'failed', cxl: 'cancelled' };
const ORDER = ['fail', 'blk', 'wait', 'rev', 'run', 'q', 'done', 'cxl'];
const SPIN = ['⠋', '⠹', '⠼', '⠧'];

// Flavours: vocabulary comes from data, never from the renderer.
const FL = {
  pr: { key: 'pr-review', label: 'PR review cycle', flow: 'author → review rN → fix rN → CI → merge', roles: { author: 'author', reviewer: 'reviewer', explorer: 'explorer', fixer: 'fixer' } },
  sp: { key: 'superpowers', label: 'superpowers SDD', flow: 'implement → task review → fix ≤5 → complete', roles: { implementer: 'implementer', reviewer: 'task-reviewer', fixer: 'fixer', explorer: 'explorer' } },
  co: { key: 'coordinator', label: 'coordinator + workers', flow: 'phases set per worker by coordinator', roles: { worker: 'worker', researcher: 'researcher', fixer: 'fixer', reviewer: 'verifier' } },
};

const ag = (id, role, model, st, age, tools, phs, act, x = {}) => ({ id, role, model, st, age, tools, phs, act, ...x });
const T = (id, name, ph, agents, x = {}) => ({ id, name, ph, agents, ...x });

export const DATA = [
  { name: 'Payments UI', tab: 1, fl: 'pr', threads: [
    T('t595', 'PR #595 soft-reuse ledger', [['author', 'd', '09:02', '09:20'], ['review r1', 'd', '09:24', '09:38', '1B 2H 3M'], ['fix r1', 'd', '09:41', '09:43'], ['review r2', 'n', '09:43', '', '0B 1H 2M so far'], ['CI', 'x'], ['merge', 'x']], [
      ag('a1', 'author', 'opus', 'q', '1h02m', 131, [0, 2], 'idle · waits for r2 findings', { msg: 'Pushed fix r1 (3 commits). Blocker fixed by locking the ledger row before reuse.' }),
      ag('a2', 'reviewer', 'opus', 'rev', '21m', 52, [1, 3], 'diffing ledger/reuse.ts against r1 fixes', { msg: 'r1 Blocker (double-spend on reuse) is fixed. Checking the High on stale TTL next.' }),
      ag('a3', 'explorer', 'haiku', 'done', '58m', 18, [0], 'mapped ledger call sites', { parent: 'a1' }),
      ag('a4', 'explorer', 'haiku', 'done', '57m', 11, [0], 'read docs/ledger.md', { parent: 'a1' }),
      ag('a5', 'explorer', 'haiku', 'done', '38m', 9, [1], 'pulled PR #571 context', { parent: 'a2' }),
    ], { find: { B: 0, H: 1, M: 2, round: 'r2' } }),
    T('t601', 'PR #601 proxy health badge', [['author', 'd', '08:10', '08:31'], ['review r1', 'd', '08:35', '08:52', '0B 1H 1M'], ['fix r1', 'd', '08:58', '09:54'], ['CI', 'f', '09:55', '10:03', '3 failing'], ['merge', 'x']], [
      ag('b1', 'author', 'sonnet', 'done', '1h54m', 88, [0, 2], 'pushed fix r1'),
      ag('b2', 'reviewer', 'sonnet', 'done', '1h29m', 29, [1], 'posted 0B 1H 1M'),
      ag('b3', 'fixer', 'sonnet', 'fail', '9m', 23, [3], 'stopped after CI failure', { since: '1m', err: 'npm test exited 1 · 3 failing in badge.spec.ts', msg: 'HealthBadge snapshot changed and 3 tests fail on a missing aria-label. Stopping per the CI-failure rule.' }),
      ag('b4', 'explorer', 'haiku', 'done', '8m', 6, [3], 'fetched CI log', { parent: 'b3' }),
    ]),
    T('t588', 'PR #588 retry banner copy', [['author', 'n', '09:58'], ['review r1', 'x'], ['CI', 'x'], ['merge', 'x']], [
      ag('c1', 'author', 'sonnet', 'wait', '6m', 14, [0], 'waiting · Banner.tsx drafted', { since: '4m', q: 1, qs: ['Use "Retrying…" or "Reconnecting…" for the banner? The design doc and the i18n keys disagree.'], msg: 'Banner component drafted; copy is blocked on wording.' }),
      ag('c2', 'explorer', 'haiku', 'done', '5m', 7, [0], 'grepped i18n keys', { parent: 'c1' }),
    ]),
  ] },
  { name: 'Data Pipeline', tab: 2, fl: 'sp', note: 'plan harvest-sync · 7 tasks', threads: [
    T('s1', 'Task 1/7: sync job schema', [['implement', 'd', '07:21', '07:40'], ['task review', 'd', '07:41', '07:58'], ['complete', 'd', '08:01', '08:01']], [
      ag('d1', 'implementer', 'sonnet', 'done', '2h43m', 46, [0], 'committed schema'),
      ag('d2', 'reviewer', 'sonnet', 'done', '2h23m', 17, [1], 'approved'),
    ]),
    T('s2', 'Task 2/7: job runner skeleton', [['implement', 'd', '08:02', '08:19'], ['task review', 'd', '08:20', '08:29'], ['fix r1', 'd', '08:31', '08:38'], ['review r2', 'd', '08:40', '08:41'], ['complete', 'd', '08:41', '08:41']], [
      ag('e1', 'implementer', 'sonnet', 'done', '2h02m', 63, [0, 2], 'fixed runner shutdown'),
      ag('e2', 'reviewer', 'sonnet', 'done', '1h44m', 24, [1, 3], 'approved r2'),
    ]),
    T('s3', 'Task 3/7: add retry cap', [['implement', 'd', '08:42', '09:03'], ['task review', 'd', '09:05', '09:16', '1 High'], ['fix r1', 'd', '09:18', '09:28'], ['review r2', 'd', '09:30', '09:32', '1 High'], ['fix r2', 'n', '09:33', '', 'round 2 of 5'], ['review r3', 'x'], ['complete', 'x']], [
      ag('f1', 'implementer', 'sonnet', 'done', '1h22m', 58, [0], 'implemented RetryPolicy.cap'),
      ag('f2', 'reviewer', 'opus', 'done', '59m', 31, [1, 3], 'r2: cap not applied to manual retries'),
      ag('f3', 'fixer', 'sonnet', 'done', '46m', 22, [2], 'fix r1 pushed'),
      ag('f4', 'fixer', 'sonnet', 'blk', '31m', 19, [4], 'paused on permission prompt', { since: '6m', perm: 'Bash(rm -rf .cache/harvest)', msg: 'I need to clear the stale retry cache to reproduce the manual-retry path.' }),
      ag('f5', 'explorer', 'haiku', 'done', '29m', 12, [4], 'located RetryPolicy usages', { parent: 'f4' }),
    ], { find: { B: 0, H: 1, M: 0, round: 'r2' } }),
    T('s4', 'Task 4/7: dedupe tick events', [['implement', 'n', '09:38'], ['task review', 'x'], ['complete', 'x']], [
      ag('g1', 'implementer', 'sonnet', 'run', '26m', 41, [0], 'running tests · pytest tests/tick -q', { msg: 'Dedup key is (farm_id, tick_ts). Running the tick suite before review.' }),
      ag('g2', 'explorer', 'haiku', 'done', '24m', 8, [0], 'read tick event schema', { parent: 'g1' }),
      ag('g3', 'explorer', 'haiku', 'run', '1m', 6, [0], 'reading src/harvest/tick.py', { parent: 'g1' }),
    ]),
    T('s5', 'Task 5/7: backoff metrics', [['implement', 'x'], ['task review', 'x'], ['complete', 'x']], [
      ag('h1', 'implementer', 'sonnet', 'q', '0s', 0, [0], 'queued · starts after task 4'),
    ]),
  ] },
  { name: 'herdr core', tab: 3, fl: 'co', threads: [
    T('w1', 'Bug: tab prefix stacking', [['explore', 'd', '09:10', '09:21'], ['reproduce', 'd', '09:22', '09:29'], ['fix', 'n', '09:30'], ['verify', 'x'], ['merge', 'x']], [
      ag('i1', 'researcher', 'haiku', 'done', '54m', 21, [0], 'traced rename path'),
      ag('i2', 'researcher', 'haiku', 'done', '52m', 16, [0], 'found 2nd prefix write in render()'),
      ag('i3', 'fixer', 'opus', 'wait', '34m', 64, [1, 2], 'paused · editing src/tui.py:812', { since: '9m', q: 2, qs: ['Strip the "[3] " prefix on rename, or only at render time? Render-time is safer but touches 4 call sites.', 'OK to update the tab-title snapshots in tests/test_tui.py?'], msg: 'Reproduced: renaming twice gives "[3] [3] build". Paused before editing.' }),
      ag('i4', 'reviewer', 'sonnet', 'q', '0s', 0, [3], 'queued · verify after fix'),
    ], { wt: 'wt/tab-prefix', pct: 72 }),
    T('w2', 'Worker: pane resize events', [['explore', 'd', '09:20', '09:41'], ['build', 'n', '09:42'], ['verify', 'x'], ['merge', 'x']], [
      ag('j1', 'worker', 'sonnet', 'run', '22m', 97, [1], 'reading src/layout/split.rs', { msg: 'Debouncing SIGWINCH to 16 ms; wiring resize into the split tree next.' }),
      ag('j2', 'researcher', 'haiku', 'done', '43m', 14, [0], 'listed SIGWINCH handlers', { parent: 'j1' }),
      ag('j3', 'researcher', 'haiku', 'cxl', '41m', 3, [0], 'cancelled · duplicate of j2', { parent: 'j1' }),
    ], { wt: 'wt/resize', pct: 45 }),
    T('w3', 'Worker: config hot-reload', [['explore', 'n', '09:52'], ['build', 'x'], ['verify', 'x'], ['merge', 'x']], [
      ag('l1', 'researcher', 'sonnet', 'run', '12m', 33, [0], 'reading src/config/watch.rs'),
      ag('l2', 'researcher', 'haiku', 'run', '11m', 19, [0], 'searching notify crate docs'),
    ], { wt: 'wt/hot-reload', pct: 18 }),
    T('w4', 'Worker: osc52 clipboard', [['explore', 'd', '08:30', '08:44'], ['build', 'd', '08:45', '09:31'], ['verify', 'd', '09:32', '09:50'], ['merge', 'd', '09:57', '09:57']], [
      ag('k1', 'worker', 'sonnet', 'done', '1h34m', 112, [0, 1], 'merged into main'),
      ag('k2', 'reviewer', 'sonnet', 'done', '32m', 27, [2], 'verified · approved'),
    ], { wt: 'wt/osc52', pct: 100 }),
  ] },
  { name: 'Billing Migrations', tab: 4, fl: 'pr', threads: [
    T('p212', 'PR #212 drop legacy invoice cols', [['author', 'd', '09:15', '09:44'], ['review r1', 'n', '09:47'], ['fix r1', 'x'], ['CI', 'x'], ['merge', 'x']], [
      ag('m1', 'author', 'opus', 'q', '49m', 76, [0], 'idle · waits for r1 findings'),
      ag('m2', 'reviewer', 'opus', 'rev', '17m', 27, [1], 'checking migration 0042 rollback'),
      ag('m3', 'explorer', 'haiku', 'done', '47m', 10, [0], 'listed invoice column readers', { parent: 'm1' }),
    ]),
    T('p214', 'PR #214 backfill currency codes', [['author', 'd', '08:50', '09:20'], ['review r1', 'd', '09:22', '09:35', '0B 0H 2M'], ['fix r1', 'd', '09:36', '09:59'], ['CI', 'n', '10:01'], ['merge', 'x']], [
      ag('n1', 'author', 'sonnet', 'run', '1h14m', 71, [0, 2, 3], 'watching CI · 14/22 jobs'),
      ag('n2', 'reviewer', 'sonnet', 'done', '42m', 18, [1], 'posted 0B 0H 2M'),
    ]),
  ] },
];

const IDX = {}, TIDX = {}; let _n = 0;
for (const or of DATA) for (const t of or.threads) { t.or = or; TIDX[t.id] = t; for (const a of t.agents) { a.t = t; a._n = _n++; IDX[a.id] = a; } }
const ALL = Object.values(IDX);

const LIVE = a => a.st !== 'done' && a.st !== 'cxl';
const NEEDS = a => a.st === 'wait' || a.st === 'blk' || a.st === 'fail';
const byUrg = (a, b) => ORDER.indexOf(a.st) - ORDER.indexOf(b.st);
const isDone = t => t.ph.every(p => p[1] === 'd');
const urgent = l => l.slice().sort(byUrg)[0];
const curIdx = t => { const i = t.ph.findIndex(p => p[1] === 'n' || p[1] === 'f'); return i < 0 ? (isDone(t) ? t.ph.length - 1 : 0) : i; };
const roleL = (t, r) => FL[t.or.fl].roles[r] || r;
const nq = l => l.reduce((s, a) => s + (a.st === 'wait' ? (a.q || 1) : 0), 0);
let FR = 0; const setFrame = f => { FR = f; };
const glyph = a => a.st === 'run' ? SPIN[(a._n + FR) % 4] : GL[a.st];
const lastPh = a => a.phs[a.phs.length - 1];

class Scr {
  constructor(w, h, th) { Object.assign(this, { w, h, th }); this.c = Array.from({ length: h }, () => Array.from({ length: w }, () => ({ ch: ' ', fg: th.fg, bg: null, b: 0 }))); }
  put(x, y, str, fg, bg, b) { for (const ch of arr(str)) { if (y >= 0 && y < this.h && x >= 0 && x < this.w) { const c = this.c[y][x]; c.ch = ch; c.fg = fg || this.th.fg; if (bg !== undefined) c.bg = bg; c.b = b ? 1 : 0; } x++; } return x; }
  bg(x, y, w, h, col) { for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (j >= 0 && j < this.h && i >= 0 && i < this.w) this.c[j][i].bg = col; }
  hl(x, y, w, col, ch = '─') { if (w > 0) this.put(x, y, ch.repeat(w), col); }
  box(x, y, w, h, col, heavy) { const [tl, tr, bl, br, hz, vt] = heavy ? ['┏', '┓', '┗', '┛', '━', '┃'] : ['┌', '┐', '└', '┘', '─', '│']; this.put(x, y, tl + hz.repeat(w - 2) + tr, col); for (let j = 1; j < h - 1; j++) { this.put(x, y + j, vt, col); this.put(x + w - 1, y + j, vt, col); } this.put(x, y + h - 1, bl + hz.repeat(w - 2) + br, col); }
  P(x, y, segs) { for (const g of segs) { if (g == null) continue; if (typeof g === 'string') x = this.put(x, y, g, this.th.fg); else x = this.put(x, y, g[0], g[1], g[3], g[2]); } return x; }
  rows() { return this.c.map(r => { const runs = []; let cur = null; for (const c of r) { const wide = c.ch.charCodeAt(0) > 127; const k = c.fg + '|' + c.bg + '|' + c.b; if (cur && cur.k === k && !wide && !cur.w) { cur.t += c.ch; cur.n++; } else { cur = { k, t: c.ch, n: 1, fg: c.fg, bg: c.bg, b: c.b, w: wide }; runs.push(cur); } } return { r: runs.map(u => ({ t: u.t, g: u.w ? '1' : undefined, st: { display: 'inline-block', flex: 'none', width: u.n + 'ch', overflow: 'hidden', textAlign: u.w ? 'center' : 'left', color: u.fg, background: u.bg || 'transparent', fontWeight: u.b ? 700 : 400 } })) }; }); }
}

function phaseGlyph(th, t, i) {
  const p = t.ph[i];
  if (p[1] === 'd') return ['✓', th.st.done.t];
  if (p[1] === 'x') return ['·', th.faint];
  if (p[1] === 'f') return ['✗', th.st.fail.t];
  const u = urgent(t.agents.filter(a => LIVE(a) && lastPh(a) === i));
  return u ? [glyph(u), th.st[u.st].t] : ['⠋', th.st.run.t];
}
function mark(th, t) {
  const o = []; const q = nq(t.agents);
  if (t.agents.some(a => a.st === 'fail')) o.push(['✗', th.st.fail.t, 1]);
  if (t.agents.some(a => a.st === 'blk')) o.push(['!', th.st.blk.t, 1]);
  if (q) o.push(['?' + q, th.st.wait.t, 1]);
  return o;
}
const markLen = m => m.reduce((s, g) => s + len(g[0]), 0);

export const H = { setFrame, Scr, IDX, TIDX, ALL, LIVE, NEEDS, byUrg, isDone, curIdx, roleL, glyph, lastPh, nq, fit, trunc, wrap, tm, dur, NOW, FL, GL, LBL, phaseGlyph, mark, markLen, len, urgent };
