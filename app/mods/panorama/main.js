import { View, div, svg } from 'gpui-kit';
import { v_flex, h_flex } from 'gpui-base';
import { agents, terminals, panes, spaces, layout, theme, shared, setShared, setAgentOrder, run, openOverlay, closeOverlay, props, window, nextCommand } from 'herdr';
import { startSidebar, consumerFeed, closeConsumer } from './feed-client.js';
import { initViewport, wheelViewport, viewport } from './viewport.js';
import { rootTarget } from './routing.js';
import { PeekCaller } from './peek-caller.js';
import { joinRoots, groups, rootSignature, harnessOf, iconProgram, spaceTotals } from './model.js';
import { rows, isOpen, needs, finished, threadMark } from './tree.js';
import { icon, mark, rootMarks, phases, lines, borderMotion } from './marks.js';
import { tile, badge, mix, stateOf, cardColors, SURFACE } from './state.js';
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
const DOCK = { fg: '#c3c7cd', hi: '#eceef1', number: '#4d535c', need: '#ffb547', fail: '#ff6b6b', hover: '#1c1e23' };
const SECTION = { 4: stateOf('wait').hue, 3: stateOf('done').hue, 2: stateOf('run').hue };
// V10 card state (TUI dock_state order, aligned with the priority sections):
// failed > permission > done > working > needs answer > idle.
function cardState(item) {
  const kinds = item.aggregate.kinds;
  if (kinds.includes('fail')) return 'fail';
  if (kinds.includes('blk')) return 'blk';
  if (item.status === 'done' && !item.aggregate.working) return 'done';
  if (item.aggregate.working) return 'run';
  return kinds.includes('wait') ? 'wait' : 'idle';
}
// The card title slot. V11 fills it with the agent's own name; today it is the tab/pane title.
const cardTitle = item => item.detail || item.stock?.program?.name || item.name;
// Line 2's only data source (today per thread; V11 switches it to per direct subagent):
// one tile per active thread, ✓n for done threads, then the counts.
function strip(item) {
  // A thread is finished when its most urgent agent is (TUI: every agent done).
  const marks = (item.root.threads ?? []).map(threadMark), tiles = marks.filter(mark => !finished(mark.status));
  const live = item.root.threads.flatMap(thread => thread.agents).filter(agent => !finished(agent.status)).length + (item.nativeChildren?.length ?? 0);
  return { tiles, done: marks.length - tiles.length, active: tiles.length, live };
}
// The running sweep moves along the card's bottom border (SPEC 4.7), under line 2.
function sweep(on, now) {
  if (!on) return [];
  const p = (now % 2400) / 2400, eased = p < .5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
  return [div().absolute().left(28).right(14).bottom(0).h(1.5).overflow_hidden()
    .child(div().absolute().left(`${(-.28 + eased * 1.28) * 100}%`).w('28%').h_full().bg(stateOf('run').hue).opacity(.85))];
}
// Header totals for the active space: ?questions, !permission, ×failed (zeros hidden).
function headerTotals(items, spaceId) {
  const seen = new Set(), entities = [];
  for (const item of items) for (const entity of [item, ...(item.nativeChildren ?? [])]) {
    const address = entity.stock?.address ?? entity.terminal;
    if (entity.space !== spaceId || address === null || address === undefined || seen.has(address)) continue;
    seen.add(address); entities.push(entity);
  }
  const total = { wait: spaceTotals(items, spaceId).questions, blk: 0, fail: 0 };
  for (const entity of entities) {
    const agents = entity.root.threads.flatMap(thread => thread.agents);
    total.fail += agents.filter(agent => agent.status === 'fail').length + entity.root.threads.filter(thread => !thread.agents.length && thread.status === 'failed').length;
    total.blk += agents.filter(agent => agent.status === 'blk').length + (entity.status === 'blocked' ? 1 : 0);
  }
  return total;
}
export default class Panorama extends View {
  init(_props, cx) {
    this.folds = new Map(); this.hovered = null; this.held = null;
    this.changed = {};
    if (props()?.sortMenu) return;
    initViewport(this, cx);
    const client = startSidebar(this, cx);
    this.peekCaller = new PeekCaller('panorama', client, () => consumerFeed(this));
    cx.spawn(async cx => { for (;;) { const command = await nextCommand(); this.peekCaller.receive(command, cx); } });
    cx.timer.every(30000, cx => cx.notify());
    cx.timer.every(250, cx => {
      const needed = this.animate && window().active && shown(layout(), 'panorama');
      if (needed && !this.motion) this.motion = cx.timer.every(50, cx => cx.notify());
      if (!needed && this.motion) { this.motion.cancel(); this.motion = null; }
    });
  }
  close(cx) { this.peekCaller?.close(); closeConsumer(this, cx); this.motion?.cancel(); this.scrollState?.release(); this.scrollFocus?.release(); }
  wheel(event, cx) { wheelViewport(this, event, cx); }
  toggle(key, signature, open, cx) {
    this.folds.set(key, { signature, open: !open }); cx.notify();
  }
  peek(item, row, cx) {
    this.peekCaller.show({ root: item.root, thread: row.thread, agent: row.agent }, row.key, false, cx);
  }
  newAgent() { openOverlay({ mod: 'new-menu', kind: 'popup', width: 250, height: 512 }); }
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
    // V10: one graph-style card per orchestrator. Line 1: tab number, harness icon, title,
    // ?N (when not already the wait badge), state badge. Line 2: thread tiles and counts.
    const pic = item.program ?? iconProgram(item.root.harness), state = cardState(item), hue = stateOf(state).hue;
    const hovered = this.hovered === item.key, docked = item.focused, colors = cardColors(state, docked);
    const fill = hovered && !docked ? mix(hue, 0x0e / 255, DOCK.hover) : colors.fill;
    const questions = item.aggregate.questions, line = strip(item), quiet = docked && open && count > 0;
    const number = item.stock?.tab_index ?? null;
    const title = cardTitle(item);
    const head = h_flex().h(25).pl(6).pr(10).items_center()
      .child(div().w(14).mr(8).flex_none().text_right().text_size(hovered ? 13 : 11).text_color(hovered ? SURFACE.dim : DOCK.number).child(hovered ? '›' : number === null ? '' : String(number)))
      .child(div().id(`icon-${item.key}`).mr(8).flex_none().when(count > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(item.key, sig, open, cx); })).child(icon(pic, 16, t.foreground)))
      .child(div().flex_1().min_w(0).truncate().text_size(12.5).when(docked, el => el.font_semibold())
        .text_color(docked ? DOCK.hi : state === 'done' ? SURFACE.dim : DOCK.fg).child(title))
      .when(hovered && Boolean(item.stock), el => el.child(div().id(`pin-${item.key}`).ml(6).p(2).rounded(4).flex_none().text_color(SURFACE.dim).hover(el => el.bg(t.hover)).on_click((event, cx) => { cx.stop_propagation(); run('terminal.pin', { terminal: item.stock.address, pinned: !item.pinned }); this.hovered = null; cx.notify(); }).child(svg(`mods/workspace-chrome/${item.pinned ? 'pin-off' : 'pin'}.svg`).size(13))))
      .child(h_flex().ml(6).flex_none().gap(4).items_center()
        .when(questions > 0 && state !== 'wait', el => el.child(tile('wait', 16, questions)))
        .child(badge(state, undefined, { questions, height: 17 })));
    const tail = h_flex().h(21).pr(10).items_center().pl(quiet || (!line.tiles.length && !line.done) ? 52 : 28)
      .when(!quiet, el => el.child(h_flex().flex_none().gap(2).items_center()
        .children(line.tiles.map(mark => tile(mark.status, 16, mark.status === 'wait' ? mark.questions : null)))
        .when(line.done > 0, el => el.child(tile('done', 16, line.done)))))
      .child(div().flex_1().min_w(0).truncate().when(!quiet && (line.tiles.length > 0 || line.done > 0), el => el.ml(6)).text_size(11.5).text_color(docked ? SURFACE.dim : SURFACE.faint)
        .child(`${line.active} active · ${line.live} agents`));
    return div().pt(4).h(52).flex_none().child(div().id(`root-${item.key}`).relative().h(48).rounded(8).border(1).border_color(colors.border).bg(hue)
      .on_hover((on, cx) => { this.hovered = on ? item.key : (this.hovered === item.key ? null : this.hovered); cx.notify(); })
      .when(Boolean(item.terminal), el => el.cursor_pointer().on_click(() => run(rootTarget(item).name, rootTarget(item).args)))
      .child(v_flex().absolute().left(3).right(0).top(0).bottom(0).rounded_r(7).bg(fill).child(head).child(tail))
      .children(sweep(item.aggregate.sweep, now)));
  }
  detailRow(item, row, now, cx) {
    const t = theme(), agent = row.agent;
    const base = h_flex().id(row.key).relative().px(8).h(26).flex_none().rounded(8).items_center().gap(0).text_size(12.5).hover(el => el.bg(t.hover)).when(this.hovered === row.key, el => el.bg(t.hover)).when(this.peekCaller?.rowKey === row.key, el => el.bg(t.raised)).cursor_pointer().children(lines(t, row.lines)).child(div().w(4).flex_none());
    if (row.kind === 'summary') return base.on_click((_event, cx) => this.toggle(row.key, row.signature, false, cx))
      .child(div().flex_none().text_color(t.dim).child(`✓ ${row.count} ${row.tasks ? 'tasks done' : 'done'}`))
      .child(div().ml(5).flex_1().truncate().text_color(t.dim).child(`· ${row.names.join(', ')}`));
    if (row.kind === 'thread') return base.on_click((_event, cx) => this.toggle(row.key, row.signature, isOpen(this.folds, row.key, row.signature, true), cx))
      .child(div().flex_1().min_w(0).truncate().font_medium().text_color(t.foreground).child(row.thread.name))
      .child(phases(t, row.thread.phases)).child(mark(t, row.mark.status, row.mark.questions));
    const harness = harnessOf(agent, item.root, row.thread), on = this.hovered === row.key;
    return base.on_click((_event, cx) => this.peek(item, row, cx)).on_hover((hovered, cx) => { this.hovered = hovered ? row.key : null; cx.notify(); })
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
    this.feed = consumerFeed(this); this.peekCaller.sync();
    const now = Date.now(), compact = narrow(), raw = agents(), all = terminals(), ps = panes(), ss = spaces();
    const current = raw.find(agent => agent.focused);
    if (!current || current.address !== this.held?.address || current.status !== 'idle') this.held = current ? { address: current.address, status: current.status } : null;
    const stock = raw.map(agent => agent.address === this.held?.address && this.held.status === 'done' && agent.status === 'idle' ? { ...agent, status: 'done' } : agent);
    for (const agent of stock) if (this.changed[agent.address]?.status !== agent.status) { this.changed[agent.address] = { status: agent.status, at: now }; }
    for (const root of this.feed?.feed?.orchestrators ?? []) {
      const key = JSON.stringify(['root', root.host, root.host === 'herdr-core' && root.terminal !== null ? root.terminal : root.pane_id]);
      if (this.changed[key]?.status !== root.herdr_status) { this.changed[key] = { status: root.herdr_status, at: now }; }
    }
    const everything = joinRoots(this.feed?.feed, stock, all, ps, ss, this.changed);
    setAgentOrder(everything.filter(item => item.stock).map(item => item.stock.address));
    // V10: the list shows the active space's orchestrators only.
    const active = ss.find(space => space.active)?.id ?? null;
    const items = everything.filter(item => item.space !== null && item.space === active);
    const totals = headerTotals(items, active);
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
      const top = !rendered.length, heading = top ? 20 : 26;
      if (!compact) append(h_flex().id(key).pl(4).pr(2).pt(top ? 2 : 8).h(heading).flex_none().gap(6).items_center().text_size(11).font_medium().cursor_pointer()
        .on_click((_event, cx) => this.toggle(key, sig, open, cx)).child(div().truncate().text_color(grouped ? t[section.tone] : SECTION[section.key] ?? SURFACE.dim).child(section.label))
        .child(div().text_color(DOCK.number).child(String(section.roots.length))).child(div().flex_1())
        .when(!open, el => el.child(svg('mods/workspace-chrome/chevron-right.svg').size(11).text_color(SURFACE.dim))), heading);
      if (!open && !compact) continue;
      for (const item of section.roots) {
        append(this.rootRow(item, items.indexOf(item), compact, now, cx), compact ? 26 : 52, !compact && item.aggregate.sweep);
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
    if (!compact) append(h_flex().id('new-agent').mt(12).h(24).pl(6).pr(10).flex_none().items_center().rounded(6).cursor_pointer().hover(el => el.bg(DOCK.hover))
      .on_click(() => this.newAgent())
      .child(div().w(14).mr(8).flex_none().text_right().text_size(13).font_bold().text_color(SURFACE.key).child('+'))
      .child(div().flex_1().text_size(12.5).text_color(SURFACE.dim).child('New agent'))
      .child(div().flex_none().text_size(11).text_color(DOCK.number).child('n')), 36);
    const errors = [...(this.feed?.errors ?? []), ...(this.peekCaller.error ? [this.peekCaller.error] : []), ...(this.writerError ? [this.writerError] : [])];
    return v_flex().size_full().min_h(0).bg(t.chrome).border_t(1).border_color(t.border).text_size(13)
      .when(!compact, el => el.child(h_flex().px(16).h(41).pt(14).pb(5).flex_none().items_center().justify_between()
        .child(div().text_size(11.5).font_medium().text_color(t.dim).child('Agents'))
        .child(h_flex().gap(8).items_center()
          .child(h_flex().mr(2).gap(7).items_center().text_size(11).font_bold()
            .when(totals.wait > 0, el => el.child(div().text_color(DOCK.need).child(`?${totals.wait}`)))
            .when(totals.blk > 0, el => el.child(div().text_color(DOCK.need).child(`!${totals.blk}`)))
            .when(totals.fail > 0, el => el.child(div().text_color(DOCK.fail).child(`×${totals.fail}`))))
          .when(errors.length || this.feed?.stale, el => el.child(div().text_size(11).text_color(t.dim).tooltip(errors.join('\n')).child(this.feed?.stale ? '◌' : '!')))
          .child(div().id('ag-sort').text_color(t.dim).hover(el => el.text_color(t.foreground))
            .on_click(() => openOverlay({ mod: 'panorama', kind: 'popup', width: 200, height: 140, props: { sortMenu: true } }))
            .child(svg(`mods/workspace-chrome/${grouped ? 'layers' : 'clock'}.svg`).size(14))))))
      .child(viewport(this, rendered, compact))
      .when(compact, el => el.child(h_flex().h(28).items_center().justify_center().child(div().id('expand-sidebar').text_size(13).text_color(t.dim).on_click(() => { const width = shared('sidebar.width') ?? 320; setShared('sidebar.collapsed', false); run('layout.resize', { id: 'sidebar', px: width }); }).child('»'))));
  }
}
