// Synthetic V10 vocabulary gallery: scratch-only smoke mod, never shipped.
import { View, div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
import { STATES, SURFACE, badge, tile, glyph, glyphRun, cardColors } from './state.js';

function card(code, selected = false, label) {
  const c = cardColors(code, selected);
  return h_flex().relative().w(196).h(28).pl(11).pr(5).items_center().rounded(8).bg(c.fill).border_1().border_color(c.border).overflow_hidden()
    .child(div().absolute().left(0).top(0).bottom(0).w(3).bg(c.edge))
    .child(div().flex_1().text_size(12).font_semibold().text_color('#eceef1').child(label ?? STATES[code].title))
    .child(badge(code, undefined, { questions: code === 'wait' ? 2 : null }));
}
function row(code) {
  return h_flex().h(34).gap(10).items_center()
    .child(div().w(56).text_size(11).text_color('#98999b').child(code))
    .child(h_flex().w(100).child(badge(code, undefined, { questions: code === "wait" ? 2 : null })))
    .child(tile(code, 18, null, { still: true })).child(tile(code, 16, null, { still: true })).child(tile(code, 14, null, { still: true }))
    .child(div().w(16).child(glyph(code)))
    .child(card(code));
}
export default class Gallery extends View {
  render() {
    const codes = Object.keys(STATES), half = Math.ceil(codes.length / 2);
    const col = list => v_flex().gap(2).children(list.map(row));
    const label = text => div().text_size(11).text_color('#98999b').child(text);
    return v_flex().size_full().bg('#131518').p(20).gap(14).text_color('#eceef1')
      .child(h_flex().items_center().gap(12).child(div().text_size(15).font_semibold().child('Panorama V10 state vocabulary'))
        .child(label('synthetic smoke fixture · state.js badge / tile 18·16·14 / glyph / cardColors')))
      .child(h_flex().gap(36).child(col(codes.slice(0, half))).child(col(codes.slice(half))))
      .child(h_flex().gap(10).items_center()
        .child(label('counts')).child(tile('wait', 16, 2)).child(tile('done', 16, 3)).child(tile('blk', 16)).child(tile('fail', 16)).child(tile('wait', 14, 1))
        .child(label('run')).child(glyphRun(['fail', 'wait', 'run', 'run', 'rev', 'done', 'done'], 6))
        .child(label('heights')).child(badge('run', undefined, { height: 20 })).child(badge('wait', undefined, { questions: 2, height: 22 }))
        .child(label('selected')).child(card('run', true, 'Selected worker')))
      .child(h_flex().gap(8).items_center().child(label('spaces row')).child(tile('fail', 18)).child(tile('blk', 18)).child(tile('wait', 18, 1)).child(tile('run', 18))
        .child(label('sidebar line 2')).child(tile('fail', 16)).child(tile('rev', 16)).child(tile('wait', 16)).child(tile('done', 16, 2)).child(div().text_size(11.5).text_color(SURFACE.dim).child('3 active · 5 agents')));
  }
}
