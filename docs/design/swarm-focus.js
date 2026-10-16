// Reference copy for herdr-panorama (the operator approved turn 10 as drawn); it draws on the anonymised swarm-tiles.js.
// Turn 10: the docked 9e panel shows whether panorama's pane has the keyboard. Its background and secondary text
// come from herdr's selected theme (config.toml [theme] name, then [theme.custom] overrides); panorama's status
// colours, tiles and pulses stay exactly as in 9e.
//   focused    background = the theme's active_row_bg (herdr's focused-row colour); text = its text; dim = subtext0, faint = overlay1
//   unfocused  background = the theme's panel_bg ("reset" or unset: the terminal's own, nothing painted);
//              text = its text; dim = overlay1, faint = overlay0 (secondary text one step dimmer)
// The docked entry's and the hover's row backgrounds stay 9e's (their text follows the same rule); tiles keep their
// own colours. Theme tokens are herdr 0.9.3's own, measured per token.
import { renderTiles } from './swarm-tiles.js';
import { TH } from './swarm-tui.js';

const th = TH.dark, ROWS = new Set([th.sel, '#171a1f']); // 9e's docked-entry and hover row backgrounds
// built-in themes: herdr 0.9.3's own values (measured), the tokens this panel reads
export const THEMES = {
  catppuccin: { panel_bg: '#181825', active_row_bg: '#313244', text: '#cdd6f4', subtext0: '#a6adc8', overlay1: '#7f849c', overlay0: '#6c7086' },
  'tokyo-night': { panel_bg: '#1a1b26', active_row_bg: '#232636', text: '#c0caf5', subtext0: '#a9b1d6', overlay1: '#697196', overlay0: '#565f89' },
};
// the operator's herdr: no theme name (catppuccin) and [theme.custom] active_row_bg = "#414868"
export const SHOWN = [
  { key: 'yours', label: 'your herdr: catppuccin + active_row_bg #414868', t: { ...THEMES.catppuccin, active_row_bg: '#414868' } },
  { key: 'tokyo', label: 'tokyo-night', t: THEMES['tokyo-night'] },
];

// 9e's rows with the panel's own colours swapped for the theme's; everything else as drawn
function themed(rows, t, focused) {
  const bg = focused ? t.active_row_bg : t.panel_bg, map = {
    [th.fg]: t.text, [th.dim]: focused ? t.subtext0 : t.overlay1, [th.faint]: focused ? t.overlay1 : t.overlay0 };
  return rows.map(r => ({ r: r.r.map(u => {
    const plain = u.st.background === 'transparent', row = plain || ROWS.has(u.st.background); // not a tile
    return { ...u, st: { ...u.st, color: row && map[u.st.color] ? map[u.st.color] : u.st.color, background: plain ? bg : u.st.background } };
  }) }));
}

export function renderFocus(f = 0) {
  const e = renderTiles(f).PE, out = {};
  for (const s of SHOWN) { out[s.key + 'A'] = themed(e, s.t, true); out[s.key + 'I'] = themed(e, s.t, false); }
  return out;
}
