import { div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
import { icon, mark, rootMarks, phases, age } from './marks.js';
import { harnessOf, iconProgram } from './model.js';
import { needs } from './tree.js';
import { cardMotion } from './graph.js';
const statusWord = status => ({ run: 'running', rev: 'reviewing', wait: 'needs answer', blk: 'blocked', fail: 'failed', done: 'finished', cxl: 'cancelled', q: 'queued', bg: 'waiting', inf: 'finished', lost: 'lost', stale: 'stale' }[status] ?? status);
export const statusTone = (t, status) => needs(status) ? t.blocked : status === 'run' ? t.working : status === 'rev' ? t.ansi5 : t.dim;
export const alpha = (color, opacity) => color.slice(0, 7) + opacity;
export function mix(background, foreground, amount) {
  const channel = (color, offset) => parseInt(color.slice(offset, offset + 2), 16);
  if (!/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(background) || !/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(foreground)) return background;
  return '#' + [1,3,5].map(offset => Math.round(channel(background, offset) * (1 - amount) + channel(foreground, offset) * amount).toString(16).padStart(2, '0')).join('');
}
const text = (value, color, size = 12) => div().min_w(0).truncate().text_size(size).text_color(color).child(value ?? '');
const stamp = (timestamp, now) => timestamp ? age(timestamp * 1000, now) : '';
function motion(t, now, edge, sweep) {
  const p = now % 2400 / 2400, eased = p < .5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
  const result = [];
  if (edge) result.push(div().absolute().left(-1.5).top(8).bottom(8).w(2).rounded(1).bg(t.blocked).opacity(.3 + .7 * (.5 - .5 * Math.cos(2 * Math.PI * now / 1600))));
  if (sweep) result.push(div().absolute().left(10).right(10).bottom(-1.5).h(2).overflow_hidden()
    .child(div().absolute().left(`${(-.28 + eased * 1.28) * 100}%`).w('28%').h_full().bg(t.working).opacity(.85)));
  return result;
}
function phaseTime(phase) {
  const time = value => value ? new Date(value * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '';
  return phase.end ? `${time(phase.start)}–${time(phase.end)}` : phase.start ? `since ${time(phase.start)}` : '';
}
export function card(view, cx, t, node, model, window, path, now) {
  const selected = view.current?.key === node.key, onPath = path.has(node.key);
  const { edge, sweep } = cardMotion(node); let body;
  if (node.kind === 'fold') {
    const roles = new Map(); node.fold.forEach(agent => roles.set(agent.role, (roles.get(agent.role) ?? 0) + 1));
    const label = `${node.fold.length} finished · ${[...roles].map(([role, count]) => count > 1 ? `${role} ×${count}` : role).join(', ')}`;
    body = h_flex().h_full().px(12).gap(3).items_center().children(node.fold.slice(0, 3).map(() => div().size(12).rounded(3).bg(t.dim)))
      .child(text(label, t.dim).ml(5).flex_1());
  } else if (node.kind === 'orch' || node.kind === 'native') {
    const item = node.item;
    const harness = item.program?.name ?? item.root.harness;
    body = v_flex().px(12).pt(9).gap(6)
      .child(h_flex().items_center().gap(7).child(icon(item.program, 16, t.foreground))
        .child(text(item.detail || item.name, t.foreground, 12.5).font_medium().flex_1()).child(rootMarks(t, item)))
      .child(text([harness, item.changed ? age(item.changed, now) : '', item.stock?.tab_index ? `tab ${item.stock.tab_index}` : ''].filter(Boolean).join(' · '), t.dim))
      .when(selected && node.kind === 'native', el => el.child(h_flex().h(24).items_center()
        .child(view.actionButton(t, 'open', 'Open pane ⏎', node, true))));
  } else if (node.kind === 'thread') {
    const thread = node.thread, phase = thread.phases.find(phase => ['now', 'failed', 'waiting'].includes(phase.state));
    const count = thread.agents.length, levels = model.tree.depth;
    body = v_flex().px(12).pt(9).gap(6)
      .child(h_flex().items_center().child(text(thread.name, t.foreground, 12.5).font_medium().flex_1()).child(mark(t, node.mark.status, node.mark.questions)))
      .child(h_flex().items_center().child(phases(t, thread.phases).ml(0))
        .child(text(selected ? `${phase?.name ?? ''} · ${stamp(thread.started, now)} · ${count} agents, ${levels} levels` : [phase?.name, phase?.note].filter(Boolean).join(' · '), needs(node.mark.status) ? t.blocked : t.dim).ml(8).flex_1()))
      .when(selected, el => el.child(v_flex().children(thread.phases.map(phase => h_flex().h(20).gap(8).items_center().text_size(12)
        .child(text(({ done: '✓', now: '▸', failed: '✗', waiting: '?', skipped: '·', next: '·' })[phase.state] ?? '·', phase.state === 'now' ? t.foreground : t.dim).w(12))
        .child(text(phase.name, phase.state === 'now' ? t.foreground : phase.state === 'done' ? t.muted : t.dim).flex_1())
        .child(text(phaseTime(phase), t.dim, 10.5))))));
  } else {
    const agent = node.agent, program = iconProgram(harnessOf(agent, model.item.root, model.thread));
    const tone = statusTone(t, agent.status), off = node.column + 1 > window.hi;
    const children = model.tree.nodes.filter(child => child.parent?.key === node.key).reduce((n, child) => n + (child.fold?.length ?? 1), 0);
    const top = h_flex().items_center().gap(6).child(icon(program, 14, t.foreground))
      .child(text(agent.role, onPath ? t.foreground : t.muted, 12.5).flex_none())
      .child(text([agent.id, agent.model].filter(Boolean).join(' · '), t.dim, 11.5).flex_1())
      .child(selected ? text(statusWord(agent.status), tone, 11.5).flex_none() : mark(t, agent.status, agent.question_ids.length));
    body = v_flex().px(12).pt(9).gap(6).child(top);
    if (!selected) body = body.child(h_flex().items_center().gap(8).child(text(agent.activity, edge ? t.blocked : t.dim).flex_1())
      .child(text(off && children ? `› ${children}` : [stamp(agent.last_at || agent.started, now), agent.tool_calls ? `${agent.tool_calls} calls` : ''].filter(Boolean).join(' · '), t.dim, 11.5).flex_none()));
    else {
      const detail = (prefix, value, color) => h_flex().gap(7).text_size(12).child(text(prefix, color).w(28).flex_none())
        .child(div().min_w(0).flex_1().line_height("17px").text_color(color).child(value));
      body = body.child(detail('now', agent.activity ?? '', t.muted))
        .children(agent.questions.map(question => detail('?', question, t.blocked)))
        .when(Boolean(agent.error), el => el.child(detail('✗', agent.error, t.blocked)))
        .child(h_flex().h(24).gap(5).items_center()
          .child(view.actionButton(t, 'open', 'Peek ⏎', node, true))
          .child(view.actionButton(t, 'transcript', 'Transcript t', node))
          .child(view.actionButton(t, 'answer', agent.status === 'blk' ? 'Allow / deny a' : 'Answer a', node, false, view.canAnswer(node))));
    }
  }
  const fill = selected ? mix(t.background, t.foreground, .045) : node.kind === 'fold' ? t.background : mix(t.background, t.foreground, .028);
  return div().relative().id(`graph-card-${node.key}`).w_full().h(node.height).rounded(8).border(1)
    .border_color(selected ? t.accent : onPath ? alpha(t.foreground, '38') : t.border).bg(fill)
    .when(node.kind === 'orch' && node.item.status === 'done', el => el.opacity(.55))
    .on_mouse_down('left', (event, cx) => { cx.stop_propagation(); view.click(node, event, cx); })
    .hover(style => style.bg(mix(t.background, t.foreground, .045))).child(body).children(motion(t, now, edge, sweep));
}
