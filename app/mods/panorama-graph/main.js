import { View, div } from 'gpui-kit';
import { h_flex, v_flex } from 'gpui-base';
import { agents, terminals, panes, spaces, focused, props, theme, run, nextCommand, settings, appHome } from 'herdr';
import { joinRoots } from './model.js';
import { startConsumer, consumerFeed, closeConsumer } from './feed-client.js';
import { InboxNavigation } from './inbox.js';
import { rootTarget } from './routing.js';
import { PeekCaller } from './peek-caller.js';
import { threadKey, agentKey } from './contract.js';

import { graphModel, columnWindow, currentNode, selectNode, navigate, selectionPath, ClickGuard, viewportAnimates, logicalSize } from './graph.js';
import { card, alpha } from './cards.js';
import { answerIntent } from './actions.js';
export default class Graph extends View {
  init(initial, cx) {
    const client = startConsumer(this, cx);
    this.peekCaller = new PeekCaller('panorama-graph', client, () => consumerFeed(this));
    this.inbox = new InboxNavigation({ callPrivate: client.callPrivate, appHome: appHome() });
    cx.spawn(async cx => { await this.inbox.refresh(); cx.notify(); });
    this.changed = {}; // Status ages belong to this live view, never a persisted store.
    this.space = initial?.space ?? props()?.space ?? focused()?.space;
    this.state = { focus: 1, orch: null, thread: null, agent: null, expanded: new Set(), remember: new Map(), manual: null, scroll: {} };
    this.guard = new ClickGuard(); this.size = { width: 1000, height: 600 }; this.handle = cx.focus_handle();
    cx.timer.every(30000, cx => { cx.notify(); cx.spawn(async cx => { if (!this.inbox.busy) { await this.inbox.refresh(); cx.notify(); } }); });
    cx.spawn(async cx => {
      for (;;) {
        const command = await nextCommand();
        if (this.peekCaller.receive(command, cx)) continue;
        if (command === 'focus' || command?.name === 'focus') {
          const pane = panes().find(pane => pane.pane === focused()?.pane && pane.mod === 'panorama-graph');
          if (pane) this.graphPane = pane.pane;
          this.handle.focus();
        }
        cx.notify();
      }
    });
  }
  isVisible() {
    const all = panes(), focus = focused();
    let own = all.find(pane => pane.mod === 'panorama-graph' && pane.pane === (this.graphPane ?? props()?.pane));
    if (!own) {
      const tabs = new Set(spaces().find(space => space.id === this.space)?.tabs.map(tab => tab.id) ?? []);
      const candidates = all.filter(pane => pane.mod === 'panorama-graph' && tabs.has(pane.tab));
      // A normal Swarm space has one graph. Ambiguous copies wait for this copy's
      // native focus command instead of assigning another pane's visibility.
      if (candidates.length === 1) { own = candidates[0]; this.graphPane = own.pane; }
    }
    return Boolean(own && own.tab === focus?.tab && !all.some(pane => pane.tab === own.tab && pane.zoomed && pane.pane !== own.pane));
  }
  model() {
    const wrapper = consumerFeed(this); this.packet = wrapper;
    const stock = agents();
    for (const agent of stock) {
      const status = agent.status === 'done' ? 'idle' : agent.status;
      if (this.changed[agent.address]?.s !== status) { this.changed[agent.address] = { s: status, at: Date.now() }; }
    }
    const roots = joinRoots(wrapper?.feed, stock, terminals(), panes(), spaces(), this.changed).filter(item => item.space === this.space);
    let model = graphModel(roots, this.state);
    if (model.item && this.state.orch !== model.item.key) this.state.orch = model.item.key;
    if (model.thread && this.state.thread !== threadKey(model.item.root, model.thread)) this.state.thread = threadKey(model.item.root, model.thread);
    this.state.focus = Math.min(this.state.focus, model.columns.length - 1);
    while (this.state.focus > 0 && !model.columns[this.state.focus]?.length) this.state.focus--;
    if (this.state.focus >= (model.nativeFallback ? 1 : 2)) {
      const current = currentNode(model, this.state);
      if (current && this.state.agent !== current.key) { this.state.agent = current.key; model = graphModel(roots, this.state); }
    }
    model.source = JSON.stringify([wrapper, roots.map(root => [root.key, root.status, root.root])]);
    this.current = currentNode(model, this.state);
    return model;
  }
  canAnswer(node) { const intent = answerIntent(this.lastModel?.item, node); return intent?.kind === 'permission' || (intent?.kind === 'question' && !this.inbox.busy && this.inbox.canOpen(intent.question)); }
  actionButton(t, action, label, node, primary = false, enabled = true) {
    return div().id(`${node.key}-${action}`).h(24).px(9).rounded(6).flex_none().flex().items_center().text_size(11.5)
      .text_color(!enabled ? t.dim : primary ? t.solid_foreground : t.foreground).bg(primary ? t.solid : t.raised)
      .on_mouse_down('left', (_e, cx) => cx.stop_propagation())
      .on_click((_e, cx) => { cx.stop_propagation(); if (enabled && this.guard.allows(node.key, Date.now())) this.act(action, node, cx); }).child(label);
  }
  act(action, node = this.current, cx) {
    if (action === 'inbox') { this.openInbox(null, cx); return; }
    if (!node || !this.guard.allows(node.key, Date.now())) return;
    const model = this.model();
    if (node.kind === 'fold' && action === 'open') { this.state.expanded.add(node.parent?.key ?? threadKey(model.item.root, model.thread)); this.state.agent = agentKey(model.item.root, node.fold[0]); return; }
    if (action === 'orchestrator') { const target = rootTarget(model.item); if (target) run(target.name, target.args); return; }
    if (action === 'answer') {
      const intent = answerIntent(model.item, node);
      if (intent?.kind === 'permission') run(intent.target.name, intent.target.args);
      else if (intent?.kind === 'question') this.openInbox(intent.question, cx);
      return;
    }
    if (action === 'transcript') {
      const item = node.item ?? model.item;
      this.peekCaller.show({ root: item.root, thread: node.thread ?? model.thread, agent: node.agent ?? null }, node.key, true, cx); return;
    }
    if (node.kind === 'orch' || node.kind === 'thread' || node.kind === 'native') {
      const target = rootTarget(node.item ?? model.item); if (target) run(target.name, target.args); return;
    }
    this.peekCaller.show({ root: model.item.root, thread: model.thread, agent: node.agent }, node.key, false, cx);
  }
  openInbox(question, cx) {
    if (!cx || !this.inbox.canOpen(question) || this.inbox.busy) return;
    cx.spawn(async cx => { const pending = this.inbox.open(question); cx.notify(); await pending; cx.notify(); });
  }
  close(cx) { this.peekCaller.close(); this.animation?.cancel(); closeConsumer(this, cx); }
  click(node, event, cx) {
    this.handle.focus();
    if (!this.guard.allows(node.key, Date.now())) return;
    const model = this.model(); selectNode(this.state, model, node);
    if (node.kind === 'fold') this.act('open', node, cx);
    else if (node.kind === 'agent' || node.kind === 'native' || event.click_count === 2) this.act('open', node, cx);
    cx.notify();
  }
  key(event, cx) {
    if (event.modifiers.control || event.modifiers.platform || event.modifiers.alt) return;
    const direction = ({ up: 'up', k: 'up', down: 'down', j: 'down', right: 'right', l: 'right', left: 'left', h: 'left' })[event.key];
    if (direction) { navigate(this.state, this.model(), direction); cx.stop_propagation(); cx.notify(); return; }
    const action = ({ enter: 'open', t: 'transcript', a: 'answer', i: 'inbox', g: 'orchestrator' })[event.key];
    if (action) { this.act(action, this.current, cx); cx.stop_propagation(); cx.notify(); }
  }
  scroll(event, cx, column = null) {
    cx.stop_propagation(); this.guard.scroll(Date.now());
    if (event.modifiers.shift || Math.abs(event.delta.x) > Math.abs(event.delta.y)) {
      const sign = Math.sign(event.modifiers.shift ? -event.delta.y : -event.delta.x);
      this.state.manual = Math.max(0, Math.min(this.model().columns.length - 1, (this.state.manual ?? this.window.lo) + sign));
    } else if (column !== null) {
      const content = Math.max(0, ...this.lastModel.columns[column].map(node => node.y + node.height));
      const maximum = Math.max(0, content - this.canvasHeight);
      this.state.scroll[column] = Math.max(0, Math.min(maximum, (this.offsets[column] ?? 0) - Math.sign(event.delta.y) * 3 * 64));
    }
    cx.notify();
  }
  offsetsFor(model, height) {
    const offsets = {};
    for (let column = 0; column < model.columns.length; column++) {
      const current = column === this.state.focus ? this.current : model.columns[column].find(node => selectionPath(model, this.state).has(node.key));
      const selectedOffset = current && current.y + current.height > height ? Math.max(0, current.y + current.height - height) : 0;
      offsets[column] = this.state.scroll[column] ?? selectedOffset;
    }
    return offsets;
  }
  connectors(t, model, window) {
    const result = [], path = selectionPath(model, this.state), columns = new Map(window.columns.map(column => [column.column, column]));
    const groupLine = (parent, column, ys) => {
      const child = columns.get(column); if (!child || !ys.length) return;
      const source = columns.get(column - 1), parentY = (parent?.y ?? 0) + (parent?.mid ?? 27) - (this.offsets[column - 1] ?? 0);
      const targetYs = ys.map(y => y - (this.offsets[column] ?? 0));
      const from = source ? source.x + source.width : 0, spine = child.x - 18;
      const targets = model.columns[column].filter(node => ys.includes(node.y + node.mid));
      const bright = alpha(t.foreground, '4d');
      const line = (x, y, width, height, color = t.input) => div().absolute().left(x).top(y).w(Math.max(1.5, width)).h(Math.max(1.5, height)).bg(color);
      result.push(line(from, parentY, Math.max(0, spine - from), 1.5, targets.some(node => path.has(node.key)) ? bright : t.input));
      const all = [parentY, ...targetYs], top = Math.min(...all), bottom = Math.max(...all);
      result.push(line(spine, top, 1.5, bottom - top));
      for (let index = 0; index < targetYs.length; index++) result.push(line(spine, targetYs[index], child.x - spine, 1.5, path.has(targets[index]?.key) ? bright : t.input));
    };
    const orch = model.orchs.find(node => node.key === model.item?.key);
    if (orch) groupLine(orch, 1, model.threadNodes.map(node => node.y + node.mid));
    for (const group of model.tree.groups) {
      const parent = group.parent ?? model.threadNodes.find(node => node.thread === model.thread);
      groupLine(parent ?? orch, group.depth + (model.nativeFallback ? 0 : 1), group.ys);
    }
    return result;
  }
  render(cx) {
    const zoom = settings().zoom;
    if (this.measuredSize) this.size = logicalSize(this.measuredSize, zoom);
    if (this.measuredCanvas) this.canvasSize = logicalSize(this.measuredCanvas, zoom);
    this.peekCaller.sync();
    const t = theme(), now = Date.now(), model = this.model(); this.lastModel = model;
    const source = model.source;
    this.guard.refresh(model.nodes, now, this.source !== undefined && source !== this.source); this.source = source;
    const window = columnWindow(model.columns.length, this.state.focus, this.size.width - 32, this.state.manual, model.nativeFallback); this.window = window;
    const height = this.canvasSize?.height ?? Math.max(0, this.size.height - 104); this.canvasHeight = height;
    const offsets = this.offsetsFor(model, height);
    const viewport = JSON.stringify([window.lo, window.hi, offsets]);
    if (this.viewport !== undefined && this.viewport !== viewport) this.guard.scroll(now);
    this.viewport = viewport; this.offsets = offsets;
    const animate = this.isVisible() && viewportAnimates(model, window, this.offsets, this.canvasSize?.width ?? this.size.width - 32, height);
    if (animate && !this.animation) this.animation = cx.timer.every(50, cx => { if (this.isVisible()) cx.notify(); else { this.animation.cancel(); this.animation = null; } });
    if (!animate && this.animation) { this.animation.cancel(); this.animation = null; }
    const path = selectionPath(model, this.state), hiddenRight = model.columns.length - 1 - window.hi;
    const crumbs = [model.item?.detail || model.item?.name, ...(model.thread ? [model.thread.name] : [])];
    let selected = model.nodes.find(node => node.key === this.state.agent); const agentPath = [];
    while (selected) { agentPath.unshift(selected.agent?.id ?? selected.item?.name ?? ''); selected = selected.parent; } crumbs.push(...agentPath);
    const label = column => column === 0 ? 'Orchestrators' : model.nativeFallback ? `Depth ${column}` : column === 1 ? 'Threads' : `Depth ${column - 1}`;
    const canvas = div().relative().flex_1().min_h(0).w_full().overflow_hidden().id('graph-canvas')
      .on_size_change((size, cx) => { if (!this.measuredCanvas || size.height !== this.measuredCanvas.height || size.width !== this.measuredCanvas.width) { this.measuredCanvas = size; cx.notify(); } })
      .on_scroll_wheel((event, cx) => this.scroll(event, cx)).children(this.connectors(t, model, window))
      .children(window.columns.map(column => div().absolute().left(column.x).top(0).bottom(0).w(column.width)
        .id(`column-${column.column}`).on_scroll_wheel((event, cx) => this.scroll(event, cx, column.column))
        .children(model.columns[column.column].filter(node => node.y + node.height >= this.offsets[column.column] && node.y <= this.offsets[column.column] + height).map(node =>
          div().absolute().left(0).right(0).top(node.y - this.offsets[column.column]).child(card(this, cx, t, node, model, window, path, now).when(this.peekCaller.rowKey === node.key, el => el.bg(t.raised)))))))
      .children(window.columns.flatMap(column => {
        const below = model.columns[column.column].filter(node => node.y >= this.offsets[column.column] + height).length;
        return below ? [div().absolute().left(column.x + 10).bottom(4).h(20).px(6).rounded(5).bg(t.raised).text_size(11).text_color(t.muted).child(`↓ ${below} more below`)] : [];
      }))
      .children([...(window.lo ? ['left'] : []), ...(hiddenRight ? ['right'] : [])].map(side => div().absolute().top(0).bottom(0).w(26)
        .when(side === 'left', el => el.left(0)).when(side === 'right', el => el.right(0))
        .children(Array.from({ length: 26 }, (_, index) => div().absolute().top(0).bottom(0).w(1).bg(t.background).opacity(1 - index / 26)
          .when(side === 'left', el => el.left(index)).when(side === 'right', el => el.right(index))))));
    return v_flex().id('graph-root').size_full().min_h(0).min_w(0).bg(t.background).pt(10).px(16).pb(8)
      .tooltip([...(this.packet?.errors ?? []), ...(this.inbox.error ? [this.inbox.error] : []), ...(this.peekCaller.error ? [this.peekCaller.error] : [])].join(' · '))
      .track_focus(this.handle).on_key_down((event, cx) => this.key(event, cx))
      .on_mouse_down('left', () => this.handle.focus())
      .on_size_change((size, cx) => { if (!this.measuredSize || size.width !== this.measuredSize.width || size.height !== this.measuredSize.height) { this.measuredSize = size; cx.notify(); } })
      .child(h_flex().h(22).flex_none().items_center().text_size(11.5)
        .child(div().flex_1().truncate().text_color(t.muted).child(window.lo ? `‹ ${crumbs.slice(0, window.lo).filter(Boolean).join(' › ')}   ${window.lo} columns left` : ''))
        .child(div().flex_none().text_color(t.dim).child(hiddenRight ? `depth ${window.hi}–${model.columns.length - 2} · ${hiddenRight} columns right ›` : '')))
      .child(div().relative().h(24).flex_none().children(window.columns.map(column => h_flex().absolute().left(column.x).w(column.width).h(24).gap(6).items_center().text_size(11.5)
        .child(div().font_medium().text_color(t.dim).child(label(column.column)))
        .child(div().text_color(t.dim).child(String(model.columns[column.column].reduce((count, node) => count + (node.fold?.length ?? 1), 0)))))))
      .child(canvas)
      .child(div().relative().h(6).mt(6).mb(6).rounded(3).flex_none().bg(alpha(t.foreground, '0a'))
        .child(div().absolute().left(`${window.total ? window.left / window.total * 100 : 0}%`).w(`${window.total ? window.shown / window.total * 100 : 100}%`).h_full().rounded(3).bg(alpha(t.foreground, '33'))))
      .child(h_flex().h(22).flex_none().gap(16).items_center().text_size(11.5).text_color(t.dim)
        .child('↑↓ move').child('→ open').child('← back')
        .when(Boolean(window.lo || hiddenRight), el => el.child('shift+wheel scroll'))
        .child('⏎ pane / peek').child('a answer').child('i inbox'));
  }
}
