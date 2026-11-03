import { div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
export function glyph(t, totals) {
  const square = color => div().size(5).rounded(1.5).bg(color);
  return v_flex().flex_none().gap(2).child(h_flex().gap(2).child(square(totals.needsCount ? t.blocked : t.dim)).child(square(totals.working ? t.working : t.dim)))
    .child(h_flex().gap(2).child(square(t.dim)).child(square(totals.done ? t.done : t.dim)));
}
export function countChip(t, count) {
  return div().h(16).min_w(16).px(4).rounded(4).flex_none().flex().items_center().justify_center().text_size(10.5).font_bold().bg(t.blocked + '2b').text_color(t.blocked).child(String(count));
}
