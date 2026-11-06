import { div, svg, image } from 'gpui-kit';
import { h_flex } from 'gpui-base';
import { iconProgram } from './model.js';
export function icon(program, size, color) {
  const p = program ?? iconProgram('unknown');
  return p.tint ? svg(p.icon).size(size).flex_none().text_color(color) : image(p.icon).size(size).flex_none();
}
export function mark(t, status, questions = 0, root = false) {
  if (status === 'run' || status === 'working') return svg('mods/workspace-chrome/spinner.svg').size(12).ml(8).mr(1).flex_none().text_color(t.working).spin(900);
  if (['rev', 'wait', 'blk', 'fail'].includes(status)) {
    const color = status === 'rev' ? t.ansi5 : t.blocked;
    return div().ml(6).h(16).min_w(16).px(4).rounded(4).flex_none().flex().items_center().justify_center().text_size(10.5).font_bold()
      .bg(status === 'fail' ? color : color + '2b').text_color(status === 'fail' ? t.chrome : color)
      .child(({ rev: '»', wait: `?${questions}`, blk: '!', fail: '✗' })[status]);
  }
  if (root && status === 'done') return div().size(7).ml(9).mr(3).flex_none().rounded_full().bg(t.done);
  if (['done', 'cxl', 'inf', 'lost', 'stale'].includes(status)) return div().ml(6).w(14).flex_none().text_size(11).text_center().text_color(t.dim).child(({ done: '✓', cxl: '-', inf: '✓?', lost: '~', stale: '◌' })[status]);
  if (['q', 'bg', 'idle', 'unknown'].includes(status) || root) return div().size(7).ml(9).mr(3).flex_none().rounded_full().border(1.5).border_color(t.idle ?? t.dim);
  return div().ml(6).w(14).flex_none().text_size(11).text_color(t.dim).child('?');
}
export function rootMarks(t, item) {
  return h_flex().flex_none().items_center()
    .children(item.aggregate.kinds.map(kind => mark(t, kind, item.aggregate.questions)))
    .when(item.status !== 'blocked', el => el.child(mark(t, item.status, 0, true)));
}
// Tree segments for one row (V7 "computed, not stitched"): │ and ├ run one straight
// vertical through the row, ├ adds a straight tick at the row centre (`height`), and only
// the last child's └ is a rounded elbow.
export function lines(t, segments, height = 26) {
  return segments.map(segment => div().relative().w(13).h_full().flex_none()
    .when(segment === '│' || segment === '├', el => el.child(div().absolute().left(6.25).top(0).bottom(0).w(1.5).bg(t.input)))
    .when(segment === '├', el => el.child(div().absolute().left(7).top(height / 2 - 0.75).w(6).h(1.5).bg(t.input)))
    .when(segment === '└', el => el.child(div().absolute().left(6.25).top(0).h(height / 2 + 0.75).w(6.75).border_l(1.5).border_b(1.5).rounded_bl(5).border_color(t.input))));
}
export function borderMotion(t, now, { edge = false, sweep = false, root = false } = {}) {
  const p = (now % 2400) / 2400, eased = p < .5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
  const opacity = .3 + .7 * (.5 - .5 * Math.cos(2 * Math.PI * now / 1600));
  const result = [];
  if (root) result.push(div().absolute().left(0).right(8).bottom(0).h(1).bg(t.border));
  if (edge) result.push(div().absolute().left(0).top(6).bottom(root ? 0 : 6).w(2).rounded(1).bg(t.blocked).opacity(opacity));
  if (sweep) result.push(div().absolute().left(0).right(8).bottom(-.25).h(1.5).overflow_hidden()
    .child(div().absolute().left(`${(-.28 + eased * 1.28) * 100}%`).w('28%').h_full().bg(t.working).opacity(.85)));
  return result;
}
export function age(since, now) {
  const minutes = Math.floor(Math.max(0, now - since) / 60000);
  return minutes < 1 ? '<1m' : minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`;
}
