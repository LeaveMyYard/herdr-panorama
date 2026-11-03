import { View, div, svg } from 'gpui-kit';
import { v_flex, h_flex } from 'gpui-base';
import { agents, terminals, panes, spaces, layout, theme, shared, setShared, setAgentOrder, run, openOverlay, closeOverlay, props, window } from 'herdr';
import { startSidebar, consumerFeed, closeConsumer } from './feed-client.js';
import { initViewport, wheelViewport, viewport } from './viewport.js';
import { rootTarget, detailTarget } from './routing.js';
import { joinRoots, groups, rootSignature, harnessOf, iconProgram } from './model.js';
import { rows, isOpen, needs } from './tree.js';
import { icon, mark, rootMarks, phases, lines, borderMotion, age } from './marks.js';
function find(node, id) {
  if (node?.id === id) return node;
  for (const child of node?.children ?? []) { const found = find(child, id); if (found) return found; }
  return null;
}
function shown(node, target, visible = true) {
  if (!node) return false;
  visible = visible && !node.hidden;
  if (node.content?.mod === target && visible) return true;
  return (node.children ?? []).some(child => shown(child, target, visible && (node.type !== 'stack' || child.id === node.active)));
}
const narrow = () => (find(layout(), 'sidebar')?.size?.px ?? Infinity) < 160;
const stop = (_event, cx) => cx.stop_propagation();
export default class Panorama extends View {
  init(_props, cx) {
    this.folds = new Map(); this.hovered = null; this.held = null;
    this.changed = {};
    if (props()?.sortMenu) return;
    initViewport(this, cx);
    startSidebar(this, cx);
    cx.timer.every(30000, cx => cx.notify());
    cx.timer.every(250, cx => {
      const needed = this.animate && window().active && shown(layout(), 'panorama');
      if (needed && !this.motion) this.motion = cx.timer.every(50, cx => cx.notify());
      if (!needed && this.motion) { this.motion.cancel(); this.motion = null; }
    });
  }
  close(cx) { closeConsumer(this, cx); this.motion?.cancel(); this.scrollState?.release(); this.scrollFocus?.release(); }
  wheel(event, cx) { wheelViewport(this, event, cx); }
  toggle(key, signature, open, cx) {
    this.folds.set(key, { signature, open: !open }); cx.notify();
  }
  peek(item, row) {
    openOverlay(detailTarget(item, row));
  }
  sortMenu(t) {
    const options = [{ id: 'priority', label: 'Priority', icon: 'clock' }, { id: 'grouped', label: 'Grouped by space', icon: 'layers' }];
    return v_flex().w_full().p(5).bg(t.popover).text_size(12.8)
      .child(div().px(8).pt(4).pb(6).text_size(11).text_color(t.dim).child('Sort agents'))
      .children(options.map(option => h_flex().id(`sort-${option.id}`).h(31).px(8).gap(9).rounded(7).items_center().text_color(t.foreground).hover(el => el.bg(t.hover))
        .on_click(() => { setShared('agents.sort', option.id); closeOverlay(props().overlay); })
        .child(svg(`mods/workspace-chrome/${option.icon}.svg`).size(14)).child(div().flex_1().child(option.label))
        .when((shared('agents.sort') ?? 'priority') === option.id, el => el.child(svg('mods/workspace-chrome/check.svg').size(14)))));
  }
  foldControl(key, signature, open, count, color, cx) {
    return h_flex().id(`fold-${key}`).h(18).ml(6).px(3).gap(3).items_center().flex_none().text_size(11).text_color(color).rounded(4).hover(el => el.bg(theme().hover))
      .on_click((event, cx) => { cx.stop_propagation(); this.toggle(key, signature, open, cx); })
      .child(svg(`mods/workspace-chrome/chevron-${open ? 'down' : 'right'}.svg`).size(11))
      .when(!open, el => el.child(String(count)));
  }
  rootRow(item, index, compact, now, cx) {
    const t = theme(), sig = rootSignature(item), open = isOpen(this.folds, item.key, sig, item.aggregate.priority === 4 || item.aggregate.priority === 2);
    const count = item.root.threads.length + item.nativeChildren.length;
    if (compact) return h_flex().id(`root-${item.key}`).h(26).px(8).flex_none().items_center().gap(4).hover(el => el.bg(t.hover))
      .when(Boolean(item.terminal), el => el.on_click(() => run(rootTarget(item).name, rootTarget(item).args)))
      .child(div().flex_1().min_w(0).text_right().text_color(t.muted).child(String(index + 1)))
      .child(mark(t, item.aggregate.kinds.length ? 'blk' : item.status, item.aggregate.questions, true));
    const pic = item.program ?? iconProgram(item.root.harness);
    return h_flex().id(`root-${item.key}`).relative().px(8).h(30).flex_none().rounded(8).items_center()
      .when(item.focused, el => el.bg(t.selected)).hover(el => el.bg(t.hover))
      .on_hover((on, cx) => { this.hovered = on ? item.key : null; cx.notify(); })
      .when(Boolean(item.terminal), el => el.cursor_pointer().on_click(() => run(rootTarget(item).name, rootTarget(item).args)))
      .child(div().id(`icon-${item.key}`).mr(9).flex_none().when(count > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(item.key, sig, open, cx); })).child(icon(pic, 16, t.foreground)))
      .child(h_flex().flex_1().min_w(0).gap(5).items_center()
        .child(div().max_w(110).truncate().text_color(item.status === 'idle' && !item.focused ? t.muted : t.foreground).child(item.name))
        .when(Boolean(item.detail) && item.detail !== item.name, el => el.child(div().flex_1().min_w(0).truncate().text_color(t.dim).child(`· ${item.detail}`))))
      .when(count > 0 && !open, el => el.child(this.foldControl(item.key, sig, open, count, item.aggregate.kinds.length ? t.blocked : item.status === 'done' ? t.done : t.dim, cx)))
      .when(this.hovered === item.key && Boolean(item.stock), el => el.child(div().id(`pin-${item.key}`).p(2).rounded(4).text_color(t.dim).hover(el => el.bg(t.hover)).on_click((event, cx) => { cx.stop_propagation(); run('terminal.pin', { terminal: item.stock.address, pinned: !item.pinned }); this.hovered = null; cx.notify(); }).child(svg(`mods/workspace-chrome/${item.pinned ? 'pin-off' : 'pin'}.svg`).size(13))))
      .when(this.hovered !== item.key && Boolean(item.changed), el => el.child(div().ml(8).flex_none().text_size(11).text_color(t.dim).child(age(item.changed, now))))
      .child(rootMarks(t, item)).children(borderMotion(t, now, { edge: item.aggregate.kinds.length > 0, sweep: item.aggregate.sweep, root: true }));
  }
  detailRow(item, row, now, cx) {
    const t = theme(), agent = row.agent;
    const base = h_flex().id(row.key).relative().px(8).h(26).flex_none().rounded(8).items_center().gap(0).text_size(12.5).hover(el => el.bg(t.hover)).when(this.hovered === row.key, el => el.bg(t.hover)).cursor_pointer().children(lines(t, row.lines)).child(div().w(4).flex_none());
    if (row.kind === 'summary') return base.on_click((_event, cx) => this.toggle(row.key, row.signature, false, cx))
      .child(div().flex_none().text_color(t.dim).child(`✓ ${row.count} ${row.tasks ? 'tasks done' : 'done'}`))
      .child(div().ml(5).flex_1().truncate().text_color(t.dim).child(`· ${row.names.join(', ')}`));
    if (row.kind === 'thread') return base.on_click((_event, cx) => this.toggle(row.key, row.signature, isOpen(this.folds, row.key, row.signature, true), cx))
      .child(div().flex_1().min_w(0).truncate().font_medium().text_color(t.foreground).child(row.thread.name))
      .child(phases(t, row.thread.phases)).child(mark(t, row.mark.status, row.mark.questions));
    const harness = harnessOf(agent, item.root, row.thread), on = this.hovered === row.key;
    return base.on_click(() => this.peek(item, row)).on_hover((hovered, cx) => { this.hovered = hovered ? row.key : null; cx.notify(); })
      .child(div().id(`icon-${row.key}`).mr(7).flex_none().when(row.children > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(row.key, row.signature, row.open, cx); })).child(icon(iconProgram(harness), 14, t.foreground)))
      .child(div().max_w(110).truncate().text_color(t.muted).child(agent.role))
      .child(div().ml(5).flex_1().min_w(0).truncate().text_color(needs(agent.status) ? t.blocked : t.dim).child(agent.activity ? `· ${agent.activity}` : ''))
      .when(row.children > 0 && !row.open, el => el.child(this.foldControl(row.key, row.signature, false, row.children, t.dim, cx)))
      .when(on, el => el.child(div().h(18).ml(6).px(6).rounded(5).flex_none().bg(t.raised).text_size(10.5).font_medium().text_color(t.foreground).child('Peek')))
      .child(mark(t, agent.status, agent.question_ids.length))
      .children(borderMotion(t, now, { edge: needs(agent.status) }));
  }
  render(cx) {
    const t = theme(); if (props()?.sortMenu) return this.sortMenu(t);
    this.feed = consumerFeed(this);
    const now = Date.now(), compact = narrow(), raw = agents(), all = terminals(), ps = panes(), ss = spaces();
    const current = raw.find(agent => agent.focused);
    if (!current || current.address !== this.held?.address || current.status !== 'idle') this.held = current ? { address: current.address, status: current.status } : null;
    const stock = raw.map(agent => agent.address === this.held?.address && this.held.status === 'done' && agent.status === 'idle' ? { ...agent, status: 'done' } : agent);
    for (const agent of stock) if (this.changed[agent.address]?.status !== agent.status) { this.changed[agent.address] = { status: agent.status, at: now }; }
    for (const root of this.feed?.feed?.orchestrators ?? []) {
      const key = JSON.stringify(['root', root.host, root.host === 'herdr-core' && root.terminal !== null ? root.terminal : root.pane_id]);
      if (this.changed[key]?.status !== root.herdr_status) { this.changed[key] = { status: root.herdr_status, at: now }; }
    }
    const items = joinRoots(this.feed?.feed, stock, all, ps, ss, this.changed);
    setAgentOrder(items.filter(item => item.stock).map(item => item.stock.address));
    let sections = groups(items);
    const grouped = shared('agents.sort') === 'grouped';
    if (grouped) {
      const mapped = new Map();
      for (const group of sections) for (const item of group.roots) {
        const key = item.pinned ? 'pinned' : item.space !== null ? `space:${item.space}` : group.key;
        if (!mapped.has(key)) mapped.set(key, { key, label: item.pinned ? 'Pinned' : item.space !== null ? item.stock.space_name : group.label, tone: 'dim', roots: [] });
        mapped.get(key).roots.push(item);
      }
      sections = [...mapped.values()];
    }
    const rendered = [];
    const append = (element, height, animated = false) => rendered.push({ element, height, animated });
    for (const section of sections) {
      const key = `section:${section.key}`, sig = JSON.stringify(section.roots.map(rootSignature));
      const open = isOpen(this.folds, key, sig, true);
      if (!compact) append(h_flex().id(key).px(8).pt(8).h(30).flex_none().gap(6).items_center().text_size(11).font_medium().cursor_pointer()
        .on_click((_event, cx) => this.toggle(key, sig, open, cx)).child(div().truncate().text_color(t[section.tone]).child(section.label))
        .child(div().text_color(t.dim).child(String(section.roots.length))).child(div().flex_1())
        .when(!open, el => el.child(svg('mods/workspace-chrome/chevron-right.svg').size(11).text_color(t.dim))), 30);
      if (!open && !compact) continue;
      for (const item of section.roots) {
        append(this.rootRow(item, items.indexOf(item), compact, now, cx), compact ? 26 : 30, !compact && (item.aggregate.kinds.length > 0 || item.aggregate.sweep));
        if (!compact) {
          const open = isOpen(this.folds, item.key, rootSignature(item), item.aggregate.priority === 4 || item.aggregate.priority === 2);
          for (const row of rows(item.root, this.folds, open)) append(this.detailRow(item, row, now, cx), 26, row.kind === 'agent' && needs(row.agent.status));
          if (open && item.stock) for (const child of item.nativeChildren ?? []) {
            const childSig = rootSignature(child), childOpen = isOpen(this.folds, child.key, childSig, child.aggregate.priority === 4 || child.aggregate.priority === 2);
            const count = child.root.threads.length, segments = Array.from({ length: child.depth + 1 }, (_, i) => i === child.depth ? '└' : '│');
            append(h_flex().id(`terminal-${child.terminal}`).relative().h(26).px(8).flex_none().items_center().text_size(12.5).gap(0).hover(el => el.bg(t.hover)).cursor_pointer()
              .on_click(() => run(rootTarget(child).name, rootTarget(child).args)).children(lines(t, segments)).child(div().w(4).flex_none())
              .child(div().id(`native-fold-${child.terminal}`).mr(7).when(count > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(child.key, childSig, childOpen, cx); })).child(icon(child.program, 14, t.foreground)))
              .child(div().flex_1().min_w(0).truncate().text_color(t.muted).child(child.name))
              .when(count > 0 && !childOpen, el => el.child(this.foldControl(child.key, childSig, childOpen, count, child.aggregate.kinds.length ? t.blocked : t.dim, cx)))
              .child(rootMarks(t, child)).children(borderMotion(t, now, { edge: child.aggregate.kinds.length > 0 })), 26, child.aggregate.kinds.length > 0);
            if (childOpen) for (const row of rows(child.root, this.folds, true)) append(this.detailRow(child, { ...row, lines: [...segments.map(() => '│'), ...row.lines] }, now, cx), 26, row.kind === 'agent' && needs(row.agent.status));
          }
        }
      }
    }
    const errors = [...(this.feed?.errors ?? []), ...(this.writerError ? [this.writerError] : [])];
    return v_flex().size_full().min_h(0).bg(t.chrome).border_t(1).border_color(t.border).text_size(13)
      .when(!compact, el => el.child(h_flex().px(16).h(41).pt(14).pb(5).flex_none().items_center().justify_between()
        .child(div().text_size(11.5).font_medium().text_color(t.dim).child('Agents'))
        .child(h_flex().gap(8).items_center()
          .when(errors.length || this.feed?.stale, el => el.child(div().text_size(11).text_color(t.dim).tooltip(errors.join('\n')).child(this.feed?.stale ? '◌' : '!')))
          .child(div().id('ag-sort').text_color(t.dim).hover(el => el.text_color(t.foreground))
            .on_click(() => openOverlay({ mod: 'panorama', kind: 'popup', width: 200, height: 140, props: { sortMenu: true } }))
            .child(svg(`mods/workspace-chrome/${grouped ? 'layers' : 'clock'}.svg`).size(14))))))
      .child(viewport(this, rendered, compact))
      .when(compact, el => el.child(h_flex().h(28).items_center().justify_center().child(div().id('expand-sidebar').text_size(13).text_color(t.dim).on_click(() => { const width = shared('sidebar.width') ?? 320; setShared('sidebar.collapsed', false); run('layout.resize', { id: 'sidebar', px: width }); }).child('»'))));
  }
}
