import { div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
import { icon, age } from './marks.js';
import { harnessOf, iconProgram } from './model.js';
import { STATES, SURFACE, stateCode, stateOf, cardColors, badge, tile } from './state.js';
import { cardMotion } from './graph.js';
export const alpha = (color, opacity) => color.slice(0, 7) + opacity;
const text = (value, color, size = 12) => div().min_w(0).truncate().text_size(size).text_color(color).child(value ?? '');
const stamp = (timestamp, now) => timestamp ? age(timestamp * 1000, now) : '';
// The card's title slot. V11 titles every agent by the name its spawner gave it; until the
// feed carries names, roots use their pane/tab title and agents their role.
export const cardTitle = node => node.kind === 'thread' ? node.thread.name : node.item ? node.item.detail || node.item.name : node.agent.name || node.agent.role;
const title = (node, color) => text(cardTitle(node), color, 12.5);
const needsState = status => ['fail', 'blk', 'wait'].includes(stateCode(status));
// The V10 graph card (state-refresh-v10 `.dn`): a 3px state edge, a state-tinted body and a
// state border; selection lifts the base and takes the pale selection border and ring.
// Every card kind uses this one frame, so a uniform agent tree can reuse it as is.
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
// Aggregate need tiles (✗ / ! / ?N), then the own-state tile; an own blocked state is its ! tile.
function needTiles(kinds, questions, own) {
  const ownBlocked = stateCode(own) === 'blk';
  return h_flex().flex_none().items_center().gap(3)
    .children(kinds.filter(kind => !(kind === 'blk' && ownBlocked)).map(kind => tile(kind, 18, kind === 'wait' ? questions : null)))
    .child(tile(own, 18));
}
const phaseHue = { done: STATES.done.hue, now: STATES.run.hue, failed: STATES.fail.hue, waiting: STATES.wait.hue };
const pips = (t, items) => h_flex().flex_none().gap(2).items_center()
  .children(items.map(phase => div().w(8).h(4).rounded(2).bg(phaseHue[phase.state] ?? t.input)));
function phaseTime(phase) {
  const time = value => value ? new Date(value * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '';
  return phase.end ? `${time(phase.start)}–${time(phase.end)}` : phase.start ? `since ${time(phase.start)}` : '';
}
export function card(view, cx, t, node, model, window, path, now) {
  const selected = view.current?.key === node.key, onPath = path.has(node.key);
  const { edge, sweep } = cardMotion(node); let body, status;
  if (node.kind === 'fold') {
    status = 'done';
    const roles = new Map(); node.fold.forEach(agent => roles.set(agent.role, (roles.get(agent.role) ?? 0) + 1));
    const label = `${node.fold.length} finished · ${[...roles].map(([role, count]) => count > 1 ? `${role} ×${count}` : role).join(', ')}`;
    // Finished-fold pods are ✓ tiles; #a8cbb2 is the design's fold title tint.
    body = h_flex().h_full().pl(12).pr(10).gap(4).items_center()
      .children(node.fold.slice(0, 4).map(agent => tile(agent.status, 16, null, { still: true })))
      .child(text(label, '#a8cbb2').ml(4).flex_1());
  } else if (node.kind === 'orch' || node.kind === 'native') {
    const item = node.item, kinds = item.aggregate?.kinds ?? [];
    status = kinds[0] ?? item.status;
    const harness = item.program?.name ?? item.root.harness;
    body = v_flex().pl(12).pr(10).pt(9).gap(6)
      .child(h_flex().h(18).items_center().gap(9).child(icon(item.program, 16, t.foreground))
        .child(title(node, t.foreground).font_medium().flex_1())
        .child(needTiles(kinds, item.aggregate?.questions ?? 0, item.status)))
      .child(text([harness, item.changed ? age(item.changed, now) : '', item.stock?.tab_index ? `tab ${item.stock.tab_index}` : ''].filter(Boolean).join(' · '), t.dim, 11.5).h(15).line_height('15px'))
      .when(selected && node.kind === 'native', el => el.child(h_flex().h(24).items_center()
        .child(view.actionButton(t, 'open', 'Open pane ⏎', node, true))));
  } else if (node.kind === 'thread') {
    const thread = node.thread, phase = thread.phases.find(phase => ['now', 'failed', 'waiting'].includes(phase.state));
    const count = thread.agents.length, levels = model.tree.depth;
    status = node.mark.status;
    body = v_flex().pl(12).pr(10).pt(9).gap(6)
      .child(h_flex().h(18).items_center().gap(6).child(title(node, t.foreground).font_medium().flex_1())
        .child(badge(status, null, { questions: node.mark.questions })))
      .child(h_flex().h(15).items_center().child(pips(t, thread.phases))
        .child(text(selected ? `${phase?.name ?? ''} · ${stamp(thread.started, now)} · ${count} agents, ${levels} levels` : [phase?.name, phase?.note].filter(Boolean).join(' · '), needsState(status) ? stateOf(status).hue : t.dim).ml(8).flex_1()))
      .when(selected, el => el.child(v_flex().children(thread.phases.map(phase => h_flex().h(20).gap(8).items_center().text_size(12)
        .child(text(({ done: '✓', now: '▸', failed: '✗', waiting: '?', skipped: '·', next: '·' })[phase.state] ?? '·', phaseHue[phase.state] ?? t.dim).w(12))
        .child(text(phase.name, phase.state === 'now' ? t.foreground : phase.state === 'done' ? t.muted : t.dim).flex_1())
        .child(text(phaseTime(phase), t.dim, 10.5))))));
  } else {
    const agent = node.agent, program = iconProgram(harnessOf(agent, model.item.root, model.thread));
    const off = node.column + 1 > window.hi, hue = stateOf(agent.status).hue, needy = needsState(agent.status);
    status = agent.status;
    const children = model.tree.nodes.filter(child => child.parent?.key === node.key).reduce((n, child) => n + (child.fold?.length ?? 1), 0);
    const top = h_flex().h(18).items_center().gap(7).child(icon(program, 14, t.foreground))
      .child(title(node, selected || onPath ? t.foreground : t.muted).flex_none())
      .child(text([agent.id, agent.model, selected ? stamp(agent.last_at || agent.started, now) : ''].filter(Boolean).join(' · '), t.dim, 10.5).flex_1())
      .child(badge(agent.status, null, { questions: agent.question_ids.length }));
    body = v_flex().pl(12).pr(10).pt(9).gap(6).child(top);
    if (!selected) body = body.child(h_flex().h(15).items_center().gap(8).child(text(agent.activity, needy ? hue : t.dim).flex_1())
      .child(text(off && children ? `› ${children}` : [stamp(agent.last_at || agent.started, now), agent.tool_calls ? `${agent.tool_calls} calls` : ''].filter(Boolean).join(' · '), t.dim, 11).flex_none()));
    else {
      const detail = (prefix, value, color, keyColor) => h_flex().gap(7).text_size(12).child(text(prefix, keyColor, 11).w(28).flex_none())
        .child(div().min_w(0).flex_1().line_height("17px").text_color(color).child(value));
      body = body.child(detail('now', agent.activity ?? '', t.muted, t.dim))
        .children(agent.questions.map(question => detail('?', question, hue, hue)))
        .when(Boolean(agent.error), el => el.child(detail('✗', agent.error, hue, hue)))
        .child(h_flex().h(24).gap(6).items_center()
          .child(view.actionButton(t, 'open', 'Peek ⏎', node, true))
          .child(view.actionButton(t, 'transcript', 'Transcript t', node))
          .child(view.actionButton(t, 'answer', agent.status === 'blk' ? 'Allow / deny a' : 'Answer a', node, false, view.canAnswer(node))));
    }
  }
  const sweepHue = stateCode(node.agent?.status) === 'rev' ? STATES.rev.hue : STATES.run.hue;
  return frame(node, status, selected, onPath, motion(now, edge, sweep, stateOf(status).hue, sweepHue), body)
    .on_mouse_down('left', (event, cx) => { cx.stop_propagation(); view.click(node, event, cx); });
}
