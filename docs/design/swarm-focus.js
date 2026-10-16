// Reference copy for herdr-panorama (the operator approved turn 10 as drawn); it draws on the anonymised swarm-tiles.js.
// Turn 10: the docked 9e panel shows whether panorama's pane has the keyboard. By default (panel_colors = "fixed",
// the operator's choice, tuned live) its background is #14161b focused and #101215 not, with panorama's own text
// colours. With panel_colors = "herdr" the background and secondary text come from herdr's selected theme
// (config.toml [theme] name, then [theme.custom] overrides), as below. Status colours, tiles and pulses stay as in 9e.
//   focused    background = panorama's own focus_bg if set, else the unfocused background moved TINT (0.15) of the
//              way toward the theme's active_row_bg (focus_tint in the app; the operator found the full colour far too
//              light); text = its text; dim = subtext0, faint = overlay1
//   unfocused  background = panorama's own panel_bg if set, else the theme's panel_bg;
//              text = its text; dim = overlay1, faint = overlay0 (secondary text one step dimmer)
// The docked entry's row background stays 9e's; the hover row is a step lighter than the panel (HOVER of the way
// toward white), so it shows on both backgrounds; their text follows the same rule. Tiles keep their own colours.
// Theme tokens are herdr 0.9.3's own, measured per token.
import { renderTiles } from './swarm-tiles.js';
import { TH } from './swarm-tui.js';

const th = TH.dark, ROWS = new Set([th.sel, '#171a1f']); // 9e's docked-entry and hover row backgrounds
const TINT = 0.15; // focus_tint's default
const HOVER = 0.04; // the hover row: this far from the panel's background toward white
const hx = c => [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16));
const toward = (a, b, k) => '#' + hx(a).map((v, i) => Math.round(v + (hx(b)[i] - v) * k).toString(16).padStart(2, '0')).join('');
// built-in themes: herdr 0.9.3's own values (measured), the tokens this panel reads
export const THEMES = {
  catppuccin: { panel_bg: '#181825', active_row_bg: '#1e1e2e', text: '#cdd6f4', subtext0: '#a6adc8', overlay1: '#7f849c', overlay0: '#6c7086' },
  'tokyo-night': { panel_bg: '#1a1b26', active_row_bg: '#232636', text: '#c0caf5', subtext0: '#a9b1d6', overlay1: '#697196', overlay0: '#565f89' },
};
// the default (fixed colours), and herdr's tokyo-night with panel_colors = "herdr"
export const SHOWN = [
  { key: 'yours', label: 'default: fixed #14161b / #101215', t: { fixed: true, panel_bg: '#101215', focus_bg: '#14161b' } },
  { key: 'tokyo', label: 'tokyo-night', t: THEMES['tokyo-night'] },
];

// 9e's rows with the panel's own colours swapped for the theme's; everything else as drawn
function themed(rows, t, focused) {
  const bg = focused ? (t.focus_bg || toward(t.panel_bg, t.active_row_bg, TINT)) : t.panel_bg, map = t.fixed ? {} : {
    [th.fg]: t.text, [th.dim]: focused ? t.subtext0 : t.overlay1, [th.faint]: focused ? t.overlay1 : t.overlay0 };
  return rows.map(r => ({ r: r.r.map(u => {
    const plain = u.st.background === 'transparent', row = plain || ROWS.has(u.st.background); // not a tile
    const back = plain ? bg : u.st.background === '#171a1f' ? toward(bg, '#ffffff', HOVER) : u.st.background;
    return { ...u, st: { ...u.st, color: row && map[u.st.color] ? map[u.st.color] : u.st.color, background: back } };
  }) }));
}

export function renderFocus(f = 0) {
  const e = renderTiles(f).PE, out = {};
  for (const s of SHOWN) { out[s.key + 'A'] = themed(e, s.t, true); out[s.key + 'I'] = themed(e, s.t, false); }
  return out;
}
