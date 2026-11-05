import { div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
import { icon, age } from './marks.js';
import { harnessOf, iconProgram } from './model.js';
import { agentTitle } from './tree.js';
import { STATES, SURFACE, stateCode, stateOf, cardColors, badge, tile } from './state.js';
import { agentMeta, cardMotion, nodeTitle } from './graph.js';
export const alpha = (color, opacity) => color.slice(0, 7) + opacity;
const text = (value, color, size = 12) => div().min_w(0).truncate().text_size(size).text_color(color).child(value ?? '');
const stamp = (timestamp, now) => timestamp ? age(timestamp * 1000, now) : '';
const needsState = status => ['fail', 'blk', 'wait'].includes(stateCode(status));
// The V10 graph card (state-refresh-v10 `.dn`): a 3px state edge, a state-tinted body and a
// state border; selection lifts the base and takes the pale selection border and ring.
function frame(node, status, selected, onPath, overlays, body) {
  const colors = cardColors(status, selected), hover = cardColors(status, true).fill;
  return div().relative().id(`graph-card-${node.key}`).w_full().h(node.height).rounded(8).border(1)
    .border_color(selected ? SURFACE.selected : onPath ? '#ffffff55' : colors.border).bg(colors.fill)
    .hover(style => style.bg(hover))
    // The edge is a left border on an inset layer, so it follows the card's rounded corners.
    .child(div().absolute().left(0).top(0).bottom(0).right(0).rounded(7).border_l(3).border_color(colors.edge))
    .when(selected, el => el.child(div().absolute().left(-4).right(-4).top(-4).bottom(-4).rounded(11).border(1).border_color(SURFACE.selected + '44')))
    .child(body).children(overlays);
}
// Needs-you pulse on the left edge and the working sweep on the bottom border, in state hues.
function motion(now, edge, sweep, edgeHue, sweepHue) {
  const p = now % 2400 / 2400, eased = p < .5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
  const result = [];
  if (edge) result.push(div().absolute().left(-1.5).top(8).bottom(8).w(2).rounded(1).bg(edgeHue).opacity(.3 + .7 * (.5 - .5 * Math.cos(2 * Math.PI * now / 1600))));
  if (sweep) result.push(div().absolute().left(10).right(10).bottom(-1.5).h(2).overflow_hidden()
    .child(div().absolute().left(`${(-.28 + eased * 1.28) * 100}%`).w('28%').h_full().bg(sweepHue).opacity(.85)));
  return result;
}
// Line 1's right side: a root session's aggregate need tiles (✗ / ! / ?N, as V11 D/D2 draw
// them on depth-0 cards), then the own state badge, which a tile never repeats.
function marks(kinds, questions, own, ownQuestions) {
  const code = stateCode(own);
  return h_flex().flex_none().items_center().gap(3)
    .children(kinds.filter(kind => kind !== code).map(kind => tile(kind, 18, kind === 'wait' ? questions : null)))
    .child(badge(own, null, { questions: ownQuestions }));
}
// What the one card shows for a root session, a native terminal child or a transcript agent.
function subject(node, model, now) {
  if (node.kind === 'agent') {
    const agent = node.agent;
    return { status: agent.status, program: iconProgram(harnessOf(agent, node.owner.root)), kinds: [], questions: 0,
      own: agent.question_ids.length, activity: agent.activity, right: [agent.model, stamp(agent.last_at || agent.started, now)] };
  }
  const item = node.item, aggregate = item.aggregate ?? { kinds: [], questions: 0 };
  return { status: item.status, program: item.program, kinds: aggregate.kinds, questions: aggregate.questions, own: item.root.inbox?.length ?? 0,
    activity: item.root.pending?.[0] || stateOf(item.status).title.toLowerCase(),
    right: [item.stock?.tab_index ? `tab ${item.stock.tab_index}` : '', item.changed ? age(item.changed, now) : ''] };
}
export function card(view, cx, t, node, model, window, path, now) {
  const selected = view.current?.key === node.key, onPath = path.has(node.key);
  const { edge, sweep } = cardMotion(node); let body, status;
  if (node.kind === 'fold') {
    status = 'done';
    const label = [node.fold.length ? `${node.fold.length} finished` : null, node.fold.map(agentTitle).join(', ') || null, node.older ? `+${node.older} older` : null].filter(Boolean).join(' · ');
    // Finished-fold pods are ✓ tiles; #a8cbb2 is the design's fold title tint.
    body = h_flex().h_full().pl(12).pr(10).gap(4).items_center()
      .children(node.fold.slice(0, 4).map(agent => tile(agent.status, 16, null, { still: true })))
      .child(text(label, '#a8cbb2').ml(4).flex_1());
  } else {
    const s = subject(node, model, now), agent = node.agent, hue = stateOf(s.status).hue;
    status = s.status;
    const top = h_flex().h(18).items_center().gap(9).child(icon(s.program, 16, t.foreground))
      .child(text(nodeTitle(node), selected || onPath ? t.foreground : t.muted, 12.5).font_medium().flex_1())
      .child(marks(s.kinds, s.questions, s.status, s.own));
    body = v_flex().pl(12).pr(10).pt(9).gap(6).child(top);
    if (!(selected && agent)) {
      // Children off the window to the right show as `› N` in place of model and age.
      const off = node.column + 1 > window.hi && node.children > 0;
      body = body.child(h_flex().h(15).items_center().gap(8).child(text(s.activity, needsState(s.status) ? hue : t.dim, 11.5).flex_1())
        .child(text(off ? `› ${node.children}` : s.right.filter(Boolean).join(' · '), t.dim, 11).flex_none()));
    }
    if (selected && node.kind === 'native') body = body.child(h_flex().h(24).items_center().child(view.actionButton(t, 'open', 'Open pane ⏎', node, true)));
    if (selected && agent) {
      const detail = (prefix, value, color, keyColor) => h_flex().items_start().gap(7).text_size(12).child(text(prefix, keyColor, 11).line_height('17px').w(30).flex_none())
        .child(div().min_w(0).flex_1().line_height('17px').text_color(color).child(value));
      // `.dn-d` and `.dn-acts`. The key sits on the first line (`items_baseline` puts it lower in
      // the host when the value wraps). The body is out of the card's flow so it reports its content's
      // height, not the card's: that report sizes the card and moves the column below it.
      body = body.pb(2).absolute().left(0).right(0).top(0).id(`graph-card-body-${node.key}`)
        .on_size_change((size, cx) => view.measureCard(node.key, size, cx))
        .child(v_flex().pt(2).gap(3).child(detail('agent', agentMeta(agent, stamp(agent.last_at || agent.started, now)), t.muted, t.dim))
          .child(detail('now', agent.activity ?? '', t.muted, t.dim))
          .children(agent.questions.map(question => detail('?', question, hue, hue)))
          .when(Boolean(agent.error), el => el.child(detail('✗', agent.error, hue, hue))))
        .child(h_flex().h(26).pt(2).gap(6).items_center()
          .child(view.actionButton(t, 'open', 'Peek ⏎', node, true))
          .child(view.actionButton(t, 'transcript', 'Transcript t', node))
          .child(view.actionButton(t, 'answer', agent.status === 'blk' ? 'Allow / deny a' : 'Answer a', node, false, view.canAnswer(node))));
    }
  }
  const sweepHue = stateCode(node.agent?.status) === 'rev' ? STATES.rev.hue : STATES.run.hue;
  return frame(node, status, selected, onPath, motion(now, edge, sweep, stateOf(status).hue, sweepHue), body)
    .on_mouse_down('left', (event, cx) => { cx.stop_propagation(); view.click(node, event, cx); });
}
