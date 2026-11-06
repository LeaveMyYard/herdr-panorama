// Generated from app/shared/state.js by scripts/app-build.
// Panorama's shared V10 state vocabulary: hues, fills, labelled badges, square
// tiles and bare glyphs. Colours are Panorama-scoped (TUI view/palette.rs ST/PB,
// as tuned in the approved state-refresh-v10 STATES table), not theme() colours.
import { div, svg } from 'gpui-kit';
import { h_flex } from 'gpui-base';

// scripts/app-build replaces the token with the owning mod's name, so each mod
// draws its own generated state-*.svg copies (mods cannot read sibling files).
export const GLYPH_DIR = 'mods/panorama-spaces/';
const S = (title, label, text, hue, fill, ink, glyph, mute = hue) => ({ title, label, text, hue, fill, ink, glyph, mute });
// code: title, badge label, text glyph, hue (ST), tile fill/ink (PB), SVG glyph, bare-glyph hue.
export const STATES = {
  run: S('Running', 'Running', '↻', '#68c2b5', '#1f625c', '#d8f2ee', 'spin'),
  rev: S('Reviewing', 'Review', '»', '#8eaee6', '#2e4f86', '#dfe8f8', 'review'),
  wait: S('Needs answer', 'Answer', '?', '#ffb547', '#f2a93b', '#1b1203', null),
  blk: S('Permission', 'Permission', '!', '#ffb547', '#f2a93b', '#1b1203', null),
  fail: S('Failed', 'Failed', '×', '#ff6b6b', '#d8363e', '#ffffff', 'x'),
  done: S('Done', 'Done', '✓', '#6c9673', '#2c6b4c', '#e3f2e9', 'check'),
  q: S('Queued', 'Queued', '◷', '#8d939d', '#2a2e35', '#d2d6dd', 'ring'),
  bg: S('Waiting', 'Waiting', '⋯', '#b39ddb', '#3b3159', '#e6def8', 'orbit'),
  cxl: S('Cancelled', 'Cancelled', '−', '#89909b', '#24272c', '#c2c7cf', 'minus', '#80868f'),
  inf: S('Inferred done', 'Inferred', '≈', '#8a9a8d', '#26312a', '#c4d3c8', 'check-q'),
  lost: S('Lost', 'Lost', '~', '#c08a8a', '#3a2526', '#e0b4b4', 'tilde'),
  stale: S('Stale', 'Stale', '◴', '#89909b', '#1b1e23', '#c2c7cf', 'dashed', '#80868f'),
  unknown: S('Unknown', 'Unknown', '◇', '#c3c7cd', '#2a2e35', '#e2e5ea', null),
  idle: S('Idle', 'Idle', '○', '#8d939d', '#23272e', '#c2c7cf', 'ring'),
};
// Fixed Panorama surfaces shared by graph and sidebar cards.
export const SURFACE = { card: '#17191d', selectedCard: '#1c2026', selected: '#b4c8e4', key: '#e2cf9c', faint: '#666c75', dim: '#80868f' };
export const NEEDS = ['fail', 'blk', 'wait'];

// Native root statuses use words; feed agents use the short codes above.
const ALIASES = { working: 'run', running: 'run', blocked: 'blk', failed: 'fail', reviewing: 'rev', waiting: 'wait', queued: 'q', cancelled: 'cxl', inferred: 'inf', background: 'bg' };
export function stateCode(status) {
  const code = ALIASES[status] ?? status;
  return Object.hasOwn(STATES, code) ? code : 'unknown';
}
export const stateOf = status => STATES[stateCode(status)];

// `hex` at `alpha` over an opaque `base`, as an opaque #rrggbb (gpui draws flat fills).
export function mix(hex, alpha, base = SURFACE.card) {
  const channel = (value, i) => parseInt(value.slice(1 + 2 * i, 3 + 2 * i), 16);
  return '#' + [0, 1, 2].map(i => Math.round(channel(hex, i) * alpha + channel(base, i) * (1 - alpha)).toString(16).padStart(2, '0')).join('');
}
// A graph/sidebar card: hue at ~5.5% over the card base, 22% border, full-hue 3px edge.
export function cardColors(status, selected = false) {
  const s = stateOf(status), base = selected ? SURFACE.selectedCard : SURFACE.card;
  return { edge: s.hue, fill: mix(s.hue, 0x0e / 255, base), border: selected ? SURFACE.selected : mix(s.hue, 0x38 / 255, base) };
}

function symbol(code, size, color) {
  const s = STATES[code];
  if (!s.glyph) return null;
  const el = svg(`${GLYPH_DIR}state-${s.glyph}.svg`).size(size).flex_none().text_color(color);
  return code === 'run' ? el.spin(900) : el;
}
// ◇ draws small in UI fonts; give it the optical size of the SVG glyphs.
const text = (value, size, color) => div().flex_none().text_size(value === '◇' ? size + 2 : size).font_weight(700).text_color(color).child(value);
const breathe = el => el.loop('opacity', { from: 0.7, to: 1, period: 1600 });

// A bare glyph in its state hue (glyph runs inside rows).
export function glyph(status, size = 11) {
  const code = stateCode(status), s = STATES[code];
  return div().size(size).flex_none().flex().items_center().justify_center()
    .child(symbol(code, size, s.mute) ?? text(code === 'wait' ? '?' : s.text, size, s.mute));
}

// The filled labelled badge: hugs its label, no fixed width.
// `questions` prefixes ?N for wait; `height` 18 (cards), 20 (Peek) or 22.
export function badge(status, label, { questions = null, height = 18 } = {}) {
  const code = stateCode(status), s = STATES[code], font = height >= 22 ? 11 : height >= 20 ? 10.5 : 10;
  const lead = symbol(code, height >= 22 ? 13 : 11, s.ink) ?? text(code === 'wait' && questions ? `?${questions}` : s.text, font, s.ink);
  return h_flex().flex_none().items_center().gap(4).h(height).pl(height >= 22 ? 8 : height >= 20 ? 7 : 6).pr(height >= 22 ? 9 : height >= 20 ? 8 : 7).rounded(5)
    .bg(s.fill).border_1().border_color(s.hue + '50').text_color(s.ink).text_size(font).font_semibold()
    .child(lead).child(div().flex_none().whitespace_nowrap().font_semibold().child(label ?? s.label));
}

// The square tile: 18 (graph), 16 (sidebar card and its first tree level) or 14. A count
// widens it to ?N, !N or ✓N. Needs-you tiles breathe 70–100% unless `still`.
export function tile(status, size = 18, count = null, { still = false } = {}) {
  const code = stateCode(status), s = STATES[code];
  const radius = size >= 18 ? 5 : size >= 16 ? 4 : 3.5, font = size >= 16 ? 10 : 9.5, mark = size >= 18 ? 13 : size >= 16 ? 12 : 11;
  const wide = Number.isInteger(count) && count > 0 && !(code === 'done' && count === 1);
  let el = h_flex().flex_none().items_center().justify_center().h(size).rounded(radius)
    .bg(s.fill).border_1().border_color(s.hue + '50').text_color(s.ink).text_size(font).font_weight(700);
  el = wide ? el.min_w(size).px(4).gap(1) : el.w(size);
  const lead = symbol(code, mark, s.ink);
  if (lead) el = el.child(lead).when(wide, e => e.child(div().flex_none().child(String(count))));
  else el = el.child(text(code === 'wait' ? `?${wide ? count : ''}` : wide ? `${s.text}${count}` : s.text, font, s.ink));
  return NEEDS.includes(code) && !still ? breathe(el) : el;
}

// A glyph run: live states most urgent first (up to `limit`), then +N✓ for finished.
const ORDER = ['fail', 'blk', 'wait', 'rev', 'run', 'bg', 'q', 'lost', 'stale', 'unknown', 'idle', 'inf', 'cxl', 'done'];
export function glyphRun(statuses, limit = 6, finished = 0, size = 11) {
  const live = statuses.map(stateCode).filter(code => code !== 'done').sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
  const done = finished + statuses.length - live.length;
  return h_flex().flex_none().items_center().gap(2)
    .children(live.slice(0, limit).map(code => glyph(code, size)))
    .when(done > 0, el => el.child(h_flex().flex_none().items_center().ml(2).text_size(10.5).font_weight(500).text_color(SURFACE.faint)
      .child(`+${done}`).child(svg(`${GLYPH_DIR}state-check.svg`).size(size - 1).flex_none().text_color(SURFACE.faint))));
}
