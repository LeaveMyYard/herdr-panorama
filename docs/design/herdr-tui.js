
// Cell-grid renderer for the herdr "New agent" modal and the empty inbox.
// Everything is drawn into a W×H grid of monospace cells: text, colour, bold.
const arr = s => Array.from(String(s));
const len = s => arr(s).length;

export const C = {
  bg: '#101215', panel: '#171a1f', border: '#80868f', text: '#eceef1', fg: '#c3c7cd',
  muted: '#80868f', dim: '#4d535c', faint: '#2c313a', ghost: '#22262d', accent: '#e2cf9c', warn: '#ffb547',
  field: '#1d2127', sel: '#2a2f37',
};

class Scr {
  constructor(w, h, bg) { Object.assign(this, { w, h }); this.hits = []; this.c = Array.from({ length: h }, () => Array.from({ length: w }, () => ({ ch: ' ', fg: C.fg, bg: bg || null, b: 0 }))); }
  put(x, y, str, fg, bg, b) { for (const ch of arr(str)) { if (y >= 0 && y < this.h && x >= 0 && x < this.w) { const c = this.c[y][x]; c.ch = ch; c.fg = fg || C.fg; if (bg !== undefined && bg !== null) c.bg = bg; c.b = b ? 1 : 0; } x++; } return x; }
  // transparent put: spaces don't overwrite
  art(x, y, lines, fg, b) { lines.forEach((l, j) => arr(l).forEach((ch, i) => { if (ch !== ' ') this.put(x + i, y + j, ch, typeof fg === 'function' ? fg(i, j, ch) : fg, null, b); })); }
  fill(x, y, w, h, col) { for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (j >= 0 && j < this.h && i >= 0 && i < this.w) { const c = this.c[j][i]; c.bg = col; c.ch = ' '; } }
  tint(x, y, w, h, col) { for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (j >= 0 && j < this.h && i >= 0 && i < this.w) this.c[j][i].bg = col; }
  // bg fills the whole box; inner fills only the inside (border cells keep what is under them)
  box(x, y, w, h, col, bg, style, inner) {
    const S = { light: '┌┐└┘─│', round: '╭╮╰╯─│', heavy: '┏┓┗┛━┃' }[style || 'round'];
    if (bg) this.fill(x, y, w, h, bg);
    if (inner) this.fill(x + 1, y + 1, w - 2, h - 2, inner);
    this.put(x, y, S[0] + S[4].repeat(w - 2) + S[1], col, bg);
    for (let j = 1; j < h - 1; j++) { this.put(x, y + j, S[5], col, bg); this.put(x + w - 1, y + j, S[5], col, bg); }
    this.put(x, y + h - 1, S[2] + S[4].repeat(w - 2) + S[3], col, bg);
  }
  P(x, y, segs, bg) { for (const g of segs) { if (g == null) continue; if (typeof g === 'string') x = this.put(x, y, g, C.fg, bg); else x = this.put(x, y, g[0], g[1], g[3] ?? bg, g[2]); } return x; }
  hit(x, y, w, h, act) { this.hits.push({ x, y, w, h, act }); }
  rows() {
    return this.c.map(r => {
      const runs = []; let cur = null;
      for (const c of r) { const k = c.fg + '|' + c.bg + '|' + c.b; if (cur && cur.k === k) { cur.t += c.ch; cur.n++; } else { cur = { k, t: c.ch, n: 1, fg: c.fg, bg: c.bg, b: c.b }; runs.push(cur); } }
      return { r: runs.map(u => ({ t: u.t, st: { display: 'inline-block', flex: 'none', width: u.n + 'ch', overflow: 'hidden', color: u.fg, background: u.bg || 'transparent', fontWeight: u.b ? 700 : 400 } })) };
    });
  }
  out() { return { rows: this.rows(), hits: this.hits, w: this.w, h: this.h }; }
}

// ───────────────────────── harnesses ─────────────────────────
// Original character art: evocative, not any product's logo.
export const HARNESS = [
  { id: 'claude', name: 'Claude Code', cmd: 'claude', col: '#e39b72',
    big: [
      '      │      ',
      '  ╲   │   ╱  ',
      '    ╲ │ ╱    ',
      ' ─────●───── ',
      '    ╱ │ ╲    ',
      '  ╱   │   ╲  ',
      '      │      ',
    ],
    small: ['╲ │ ╱', '──●──', '╱ │ ╲'] },
  { id: 'codex', name: 'Codex', cmd: 'codex', col: '#9db6ea',
    big: [
      '             ',
      '    ╭───╮    ',
      ' ╭──╯   ╰─╮  ',
      '╭╯ ›_      ╰╮',
      '╰───────────╯',
      '             ',
      '             ',
    ],
    small: [' ╭─╮ ', '╭╯›_╰╮', '╰────╯'] },
  { id: 'opencode', name: 'OpenCode', cmd: 'opencode', col: '#c9ced6',
    big: [
      '  ╭─     ─╮  ',
      '  │ ── ─  │  ',
      '  │ ───── │  ',
      '──┤  ── ─ ├──',
      '  │ ────  │  ',
      '  │ ─ ─── │  ',
      '  ╰─     ─╯  ',
    ],
    small: ['╭─  ─╮', '┤ ── ├', '╰─  ─╯'] },
  { id: 'gemini', name: 'Gemini CLI', cmd: 'gemini', col: '#b4a6f0',
    big: [
      '     ╻     ✦ ',
      '     ┃       ',
      '    ╱ ╲      ',
      ' ━━━   ━━━   ',
      '    ╲ ╱      ',
      '     ┃       ',
      '     ╹       ',
    ],
    small: ['  ╻ ✦', '━━◆━━', '  ╹  '] },
  { id: 'droid', name: 'Droid', cmd: 'droid', col: '#8fd0a4',
    big: [
      '   ╷     ╷   ',
      ' ╭─┴─────┴─╮ ',
      ' │  ◉   ◉  │ ',
      ' │   ───   │ ',
      ' ╰─┬─────┬─╯ ',
      '  ╶┴╴   ╶┴╴  ',
      '             ',
    ],
    small: ['╭┴─┴╮', '│◉ ◉│', '╰───╯'] },
];
const NH = HARNESS.length;
export const step = (i, d) => (i + d + NH) % NH;

const NAME = 'fix tab prefix stacking';
const SLUG = 'fix-tab-prefix-stacking';
const CWD = '~/Code/herdr';
const HINT = 'enter on the last field or ctrl+s: start · tab: next field · esc: cancel';

// dimmed swarm view behind the modal
function backdrop(s) {
  const lines = [
    [' swarm ', '  4 orgs · 14 live · 2 need you', '', '10:04'],
    [],
    ['▾ Web Frontend', '  pr-review'],
    ['  ⠋ PR #595 soft-reuse ledger', '   review r2 · reviewer'],
    ['  ✗ PR #601 proxy health badge', '   CI · 3 failing'],
    ['  ? PR #588 retry banner copy', '   author · needs answer'],
    [],
    ['▾ Data Pipeline', '  superpowers'],
    ['  ✓ Task 1/7: sync job schema'],
    ['  ✓ Task 2/7: job runner skeleton'],
    ['  ! Task 3/7: add retry cap', '   fix r2 · blocked'],
    ['  ⠋ Task 4/7: dedupe tick events', '   implement'],
    ['  ○ Task 5/7: backoff metrics'],
    [],
    ['▾ Core Library', '  coordinator'],
    ['  ? Bug: tab prefix stacking', '   fix · needs answer'],
    ['  ⠋ Worker: pane resize events', '   build'],
    ['  ⠋ Worker: config hot-reload', '   explore'],
    ['  ✓ Worker: osc52 clipboard'],
    [],
    ['▾ Payments API', '  pr-review'],
    ['  » PR #212 drop legacy invoice cols', '   review r1'],
    ['  ⠋ PR #214 backfill currency codes', '   CI 14/22'],
  ];
  s.fill(0, 0, s.w, 1, '#14161a');
  lines.forEach((l, y) => { if (!l.length) return; let x = s.put(1, y, l[0], y === 0 ? '#3a3f47' : '#353a42', y === 0 ? '#14161a' : null); if (l[1]) s.put(x, y, l[1], '#2a2e35'); if (l[3]) s.put(s.w - 1 - len(l[3]), y, l[3], '#353a42'); });
  s.put(1, s.h - 1, 'n new agent · enter open · / filter · q quit', '#2a2e35');
}

// a text input: label line + box (+ sub line); returns the next free row after a one-row gap
function input(s, x, y, w, label, value, o = {}) {
  s.put(x, y, label, o.focus ? C.text : C.muted, null, o.focus);
  const bc = o.focus ? C.accent : C.dim;
  s.box(x, y + 1, w, 3, bc, null, 'light', C.field);
  let cx = s.put(x + 2, y + 2, value, C.text, C.field);
  if (o.cursor) s.put(cx, y + 2, ' ', C.bg, C.text);
  if (o.sub) s.P(x + 2, y + 4, o.sub);
  return y + 5 + (o.sub ? 1 : 0);
}
const nameSub = [['agent name: ', C.dim], [SLUG, C.muted]];

function modalFrame(s, x, y, w, h) {
  s.box(x, y, w, h, C.border, C.panel, 'round');
  s.P(x + 2, y, [[' + New agent ', C.text, 1]], C.panel);
}
function footer(s, x, y, w) {
  // y: status line (empty until there is something to say); y + 1: key hints
  s.put(x + 2, y + 1, HINT, C.dim, C.panel);
}

// ── Variant A: tab strip, art on the left ──
export function modalA(sel, f) {
  const W = 96, H = 30, s = new Scr(W, H, C.bg); backdrop(s);
  const mw = 76, mh = 19, mx = (W - mw) >> 1, my = 5;
  modalFrame(s, mx, my, mw, mh);
  let x = mx + 3; const ty = my + 2;
  HARNESS.forEach((h, i) => {
    const on = i === sel; const lab = ` ${h.name} `;
    const x0 = x;
    x = s.put(x, ty, lab, on ? '#14161a' : C.muted, on ? h.col : C.panel, on);
    s.hit(x0, ty, len(lab), 1, { set: i });
    if (i < NH - 1) x = s.put(x, ty, ' ', C.dim, C.panel);
  });
  s.put(mx + mw - 9, ty, '←/→ ', C.dim, C.panel);
  s.put(mx + 1, ty + 1, '─'.repeat(mw - 2), C.faint, C.panel);
  const h = HARNESS[sel];
  const ax = mx + 3, ay = ty + 3;
  s.box(ax, ay, 19, 11, C.faint, null, 'round', '#13161a');
  s.art(ax + 3, ay + 1, h.big, h.col, 1);
  const cmd = '$ ' + h.cmd;
  s.put(ax + ((19 - len(cmd)) >> 1), ay + 9, cmd, C.muted, '#13161a');
  const fx = ax + 22, fw = mw - 28;
  let y = ay;
  y = input(s, fx, y, fw, 'Name (also the tab label)', NAME, { sub: nameSub });
  y = input(s, fx, y, fw, 'Working directory', CWD, {});
  footer(s, mx, my + mh - 3, mw);
  return s.out();
}

// ── Variant B: vertical picker list + large art ──
export function modalB(sel, f) {
  const W = 96, H = 30, s = new Scr(W, H, C.bg); backdrop(s);
  const mw = 76, mh = 22, mx = (W - mw) >> 1, my = 4;
  modalFrame(s, mx, my, mw, mh);
  const lx = mx + 3, ly = my + 2;
  s.put(lx, ly, 'Harness', C.text, C.panel, 1);
  s.put(lx + 8, ly, '↑/↓', C.dim, C.panel);
  HARNESS.forEach((h, i) => {
    const on = i === sel, y = ly + 2 + i * 2;
    if (on) s.tint(lx - 1, y, 24, 1, C.sel);
    s.P(lx, y, [[on ? '▌' : ' ', h.col], [' ' + h.name.padEnd(13), on ? C.text : C.muted, on], [h.cmd.padStart(8) + ' ', on ? C.muted : C.dim]], on ? C.sel : C.panel);
    s.hit(lx - 1, y, 24, 1, { set: i });
  });
  const h = HARNESS[sel];
  const sx = lx + 27, sy = ly, sw = mw - 31, sh = 11;
  s.fill(sx, sy, sw, sh, '#13161a');
  s.put(sx + 2, sy + sh - 2, '·'.repeat(sw - 4).split('').map((c, i) => (i % 3 ? ' ' : '·')).join(''), C.faint, '#13161a');
  s.art(sx + ((sw - 13) >> 1), sy + 1, h.big, h.col, 1);
  s.put(sx + sw - len(h.name) - 2, sy + sh - 1, h.name, h.col, '#13161a', 1);
  const fy = ly + 13;
  input(s, lx, fy - 1, 34, 'Name (also the tab label)', NAME, { sub: nameSub });
  input(s, lx + 36, fy - 1, mw - 42, 'Working directory', CWD, {});
  footer(s, mx, my + mh - 3, mw);
  return s.out();
}

// ── Variant C: carousel, art centred above the fields ──
export function modalC(sel, f) {
  const W = 96, H = 32, s = new Scr(W, H, C.bg); backdrop(s);
  const mw = 76, mh = 27, mx = (W - mw) >> 1, my = 3;
  modalFrame(s, mx, my, mw, mh);
  const cx = mx + (mw >> 1);
  const h = HARNESS[sel], prev = HARNESS[step(sel, -1)], next = HARNESS[step(sel, 1)];
  const ay = my + 2;
  s.art(mx + 4, ay + 1, prev.big, '#2e333b');
  s.art(mx + mw - 17, ay + 1, next.big, '#2e333b');
  s.art(cx - 6, ay, h.big, h.col, 1);
  const ry = ay + 8;
  s.hit(mx + 1, ay, 18, 9, { d: -1 }); s.hit(mx + mw - 19, ay, 18, 9, { d: 1 });
  s.put(cx - 18, ry, '‹', C.accent, C.panel, 1);
  s.put(cx + 18, ry, '›', C.accent, C.panel, 1);
  s.hit(cx - 20, ry, 4, 1, { d: -1 }); s.hit(cx + 17, ry, 4, 1, { d: 1 });
  s.put(cx - (len(h.name) >> 1), ry, h.name, C.text, C.panel, 1);
  const dots = HARNESS.map((_, i) => (i === sel ? '●' : '·')).join(' ');
  s.put(cx - (len(dots) >> 1), ry + 1, dots, C.dim, C.panel);
  s.put(cx - (len(dots) >> 1) + sel * 2, ry + 1, '●', h.col, C.panel);
  s.put(mx + 1, ry + 3, '─'.repeat(mw - 2), C.faint, C.panel);
  let y = ry + 4;
  y = input(s, mx + 3, y, mw - 6, 'Name (also the tab label)', NAME, { sub: nameSub });
  y = input(s, mx + 3, y, mw - 6, 'Working directory', CWD, {});
  footer(s, mx, my + mh - 3, mw);
  return s.out();
}
// ── Variant C, Claude Code only: every harness stays in the carousel; the unsupported ones say "Coming soon" ──
const SUPPORTED = new Set(['claude']);
export const soon = h => !SUPPORTED.has(h.id);
const SOON = 'Coming soon';
const START = 'enter on the last field or ctrl+s: start';
export function modalCSoon(sel, f) {
  const W = 96, H = 32, s = new Scr(W, H, C.bg); backdrop(s);
  const mw = 76, mh = 27, mx = (W - mw) >> 1, my = 3;
  modalFrame(s, mx, my, mw, mh);
  const cx = mx + (mw >> 1);
  const h = HARNESS[sel], prev = HARNESS[step(sel, -1)], next = HARNESS[step(sel, 1)];
  const off = soon(h);
  const ay = my + 2;
  s.art(mx + 4, ay + 1, prev.big, '#2e333b');
  s.art(mx + mw - 17, ay + 1, next.big, '#2e333b');
  // a ghosted neighbour that is not supported yet carries the tag under its art
  if (soon(prev)) s.put(mx + 10 - (len(SOON) >> 1), ay + 8, SOON, '#3a3f47', C.panel);
  if (soon(next)) s.put(mx + mw - 11 - (len(SOON) >> 1), ay + 8, SOON, '#3a3f47', C.panel);
  // centre card: an unsupported harness keeps its art but drawn grey, not in its colour
  s.art(cx - 6, ay, h.big, off ? C.dim : h.col, !off);
  const ry = ay + 8;
  s.hit(mx + 1, ay, 18, 9, { d: -1 }); s.hit(mx + mw - 19, ay, 18, 9, { d: 1 });
  s.put(cx - 18, ry, '‹', C.accent, C.panel, 1);
  s.put(cx + 18, ry, '›', C.accent, C.panel, 1);
  s.hit(cx - 20, ry, 4, 1, { d: -1 }); s.hit(cx + 17, ry, 4, 1, { d: 1 });
  s.put(cx - (len(h.name) >> 1), ry, h.name, off ? C.muted : C.text, C.panel, !off);
  const dots = HARNESS.map((_, i) => (i === sel ? '●' : '·')).join(' ');
  s.put(cx - (len(dots) >> 1), ry + 1, dots, C.dim, C.panel);
  s.put(cx - (len(dots) >> 1) + sel * 2, ry + 1, '●', off ? C.muted : h.col, C.panel);
  if (off) s.put(cx - (len(SOON) >> 1), ry + 2, SOON, C.accent, C.panel);
  s.put(mx + 1, ry + 3, '─'.repeat(mw - 2), C.faint, C.panel);
  let y = ry + 4;
  y = input(s, mx + 3, y, mw - 6, 'Name (also the tab label)', NAME, { sub: nameSub });
  y = input(s, mx + 3, y, mw - 6, 'Working directory', CWD, {});
  // key hints: on an unsupported harness the start part is greyed out, the rest unchanged
  const fy = my + mh - 2;
  const x1 = s.put(mx + 2, fy, START, off ? C.faint : C.dim, C.panel);
  s.put(x1, fy, HINT.slice(START.length), C.dim, C.panel);
  return s.out();
}

// ── Variant D: compact, small art badge inside a select field ──
export function modalD(sel, f) {
  const W = 96, H = 29, s = new Scr(W, H, C.bg); backdrop(s);
  const mw = 76, mh = 22, mx = (W - mw) >> 1, my = 3;
  modalFrame(s, mx, my, mw, mh);
  const x = mx + 3, fw = mw - 6;
  let y = my + 2;
  const h = HARNESS[sel];
  s.put(x, y, 'Harness', C.text, C.panel, 1);
  s.box(x, y + 1, fw, 5, C.accent, null, 'light', C.field);
  s.art(x + 3, y + 2, h.small, h.col, 1);
  s.put(x + 11, y + 3, h.name, C.text, C.field, 1);
  s.put(x + 12 + len(h.name), y + 3, h.cmd, C.dim, C.field);
  const pg = `‹ ${sel + 1}/${NH} ›`;
  const px = x + fw - 3 - len(pg);
  s.put(px, y + 3, pg, C.accent, C.field);
  s.hit(px, y + 1, 3, 5, { d: -1 }); s.hit(px + len(pg) - 2, y + 1, 4, 5, { d: 1 });
  y += 7;
  y = input(s, x, y, fw, 'Name (also the tab label)', NAME, { sub: nameSub });
  y = input(s, x, y, fw, 'Working directory', CWD, {});
  footer(s, mx, my + mh - 3, mw);
  return s.out();
}

// ───────────────────────── empty inbox ─────────────────────────
const IW = 64, IH = 20;
function popup(s) {
  s.box(0, 0, IW, IH, C.border, C.panel, 'round');
  s.put(2, 0, ' inbox ', C.text, C.panel, 1);
}
const centre = (s, y, t, col, b) => s.put((IW - len(t)) >> 1, y, t, col, C.panel, b);

export function inboxA(f) { // empty mailbox, flag down
  const s = new Scr(IW, IH, C.panel); popup(s);
  const art = [
    '     ╭─────────────╮      ',
    '    ╭╯             ╰╮     ',
    '    │               │     ',
    '    │               │━┓   ',
    '    │               │ ┃   ',
    '    ╰───────┬───────╯ ┃   ',
    '            │         ▀   ',
    '            │             ',
    '    ┈┈┈┈┈┈┈┈┴┈┈┈┈┈┈┈┈     ',
  ];
  const ox = (IW - 26) >> 1, oy = 3;
  s.art(ox, oy, art, (i, j, ch) => (/[━┓┃▀]/.test(ch) ? '#a0554d' : j === 8 ? C.dim : C.muted));
  s.tint(ox + 6, oy + 2, 13, 3, '#0c0e11');
  s.put(ox + 9, oy + 3, '·  ·  ·', C.faint, '#0c0e11');
  centre(s, 15, 'No open questions', C.text);
  return s.out();
}

export function inboxB(f) { // sleeping cat
  const s = new Scr(IW, IH, C.panel); popup(s);
  const art = [
    '      ╱╲     ╱╲                 ',
    '     ╱  ╲───╱  ╲                ',
    '    │   ─     ─  │╲             ',
    '    │      ω     │ ╲────────╮   ',
    '     ╲__________╱            │  ',
    '      ╰─────────────────────╯~  ',
  ];
  const ox = 15, oy = 6;
  s.art(ox, oy, art, C.muted);
  const t = f % 12;
  [[0, 'z'], [4, 'z'], [8, 'Z']].forEach(([d, ch]) => {
    const p = (t + 12 - d) % 12; if (p > 7) return;
    const col = p < 3 ? C.accent : p < 6 ? C.muted : C.dim;
    s.put(ox + 20 + p, oy - 1 - (p >> 1), p > 4 ? 'Z' : ch, col, C.panel, p < 3);
  });
  s.put(ox + 3, oy + 6, '▔'.repeat(28), C.faint);
  centre(s, 15, 'No open questions', C.text);
  return s.out();
}

export function inboxC(f) { // tumbleweed in the desert
  const s = new Scr(IW, IH, C.panel); popup(s);
  const gy = 12;
  s.art(22, 3, [' ▄▄ ', '▀██▀'], '#c9a861');
  s.art(47, gy - 2, ['   ▁▄▄▄▄▄▁', ' ▁▟███████▙▁'], '#2a2e35');
  s.art(40, gy - 5, ['  ╷  ', '╷ │  ', '╰─┤ ╷', '  ├─╯', '  │  '], '#6c9673', 1);
  s.put(1, gy, '▁'.repeat(IW - 2), C.dim, C.panel);
  s.put(3, gy + 1, ' ·    ˙       ·         ˙     ·      ·  ˙        ·   ', C.faint, C.panel);
  const frames = [
    [' ⡠⠔⠒⢄ ', '⢸⠡⡈⢂⠜⡆', ' ⠑⠢⠔⠊ '],
    [' ⡔⠒⠢⢄ ', '⢸⠑⡄⡠⢁⡇', ' ⠣⠤⠔⠉ '],
    [' ⢀⠔⠒⢤ ', '⡜⢂⠌⡢⠈⡇', ' ⠈⠢⠤⠚ '],
  ];
  const span = 34, p = f % (span + 8);
  const tx = 2 + p, bounce = p % 4 === 1 ? 1 : 0;
  s.art(tx, gy - 3 - bounce, frames[p % 3], '#b49a6a', 1);
  if (p > 2) s.put(tx - 3, gy - 1, '˙ ·', C.faint);
  centre(s, 15, 'No open questions', C.text);
  return s.out();
}

export function inboxD(f) { // empty tray, all caught up
  const s = new Scr(IW, IH, C.panel); popup(s);
  const art = [
    '╭─────────────────────╮',
    '│                     │',
    '│                     │',
    '│                     │',
    '├──────╮       ╭──────┤',
    '│      ╰───────╯      │',
    '╰─────────────────────╯',
  ];
  const ox = (IW - 23) >> 1, oy = 4;
  s.art(ox, oy, art, C.muted);
  s.art(ox + 9, oy + 1, ['    ╱', '╲  ╱ ', ' ╲╱  '], C.accent, 1);
  centre(s, 14, 'All caught up', C.text, 1);
  centre(s, 15, 'No open questions', C.dim);
  return s.out();
}

export const dump = o => o.rows.map(r => r.r.map(u => u.t).join('')).join('\n');
