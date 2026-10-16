// Turn 8: smooth animation. F is the time in design frames of 250 ms; the page advances it at 20 fps
// (COLOUR_MS), so between two whole frames F is fractional. Whole steps (Math.floor(F)) move spinners
// and sweeps; colours follow F itself: the unseen pulse is already a sine of F, and the attention
// blink eases with a sine ramp instead of switching. At whole frames every colour is exactly the old
// stepped one (blink: on, on, off, off), so the golden frames do not change.
export const COLOUR_MS = 50; // 20 fps, the shipped default (config animation_fps, 2..30)
const hex = c => [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16));
export const mix = (a, b, k) => '#' + hex(a).map((v, i) => Math.round(v + (hex(b)[i] - v) * k).toString(16).padStart(2, '0')).join('');
// 1 on .. 0 off over one second: on for a frame, a sine ease off over the next, off for a frame, ease back on
export function blinkK(F) {
  const p = ((F % 4) + 4) % 4;
  if (p <= 1) return 1;
  if (p < 2) return (1 + Math.cos(Math.PI * (p - 1))) / 2;
  if (p <= 3) return 0;
  return (1 - Math.cos(Math.PI * (p - 3))) / 2;
}
// colour c blinking against the theme's faint colour
export const blinkCol = (th, c, F) => { const k = blinkK(F); return k >= 1 ? c : k <= 0 ? th.faint : mix(th.faint, c, k); };
