import { View, div, svg } from 'gpui-kit';
import { v_flex, h_flex } from 'gpui-base';
import { agents, terminals, panes, spaces, layout, theme, shared, setShared, setAgentOrder, run, openOverlay, closeOverlay, props, window, nextCommand, appHome, readTextFile } from 'herdr';
import { startSidebar, consumerFeed, closeConsumer } from './feed-client.js';
import { initViewport, wheelViewport, viewport } from './viewport.js';
import { rootTarget } from './routing.js';
import { PeekCaller } from './peek-caller.js';
import { joinRoots, groups, listed, rootSignature, iconProgram, headerTotals, storedScope, storedResume, resumeSummary, cardPlace, WAITING } from './model.js';
import { rows, isOpen, needs, directTiles, agentTitle } from './tree.js';
import { icon, mark, rootMarks, lines } from './marks.js';
import { tile, badge, glyph, glyphRun, mix, stateOf, cardColors, SURFACE } from './state.js';
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
const ROW = 20; // V9/V11 dense tree rows
const TILE = 16; // The boxed state icon, the same on line 2 and in the tree's first level.
const CORNER = 8; // The card's outer radius.
// The docked list's trunk: under the card icon's centre (edge 3 + pl 6 + number 14 + gap 8 + half the 16px icon), minus half the line.
const TRUNK = 3 + 6 + 14 + 8 + 8 - 0.75;
const DOCK = { fg: '#c3c7cd', hi: '#eceef1', number: '#4d535c', need: '#ffb547', fail: '#ff6b6b', hover: '#1c1e23' };
const SECTION = { 4: stateOf('wait').hue, 3: stateOf('done').hue, 2: stateOf('run').hue, [WAITING]: stateOf('bg').hue };
// V10 card state, aligned with the priority sections (groups()): failed > permission >
// done (own status, no need kinds, i.e. groups() priority 3) > working > needs answer > idle.
function cardState(item) {
  if (item.stale) return 'stale';
  if (item.status === 'bg') return 'bg'; // joinRoots: quiet, with live background work.
  const kinds = item.aggregate.kinds;
  if (kinds.includes('fail')) return 'fail';
  if (kinds.includes('blk')) return 'blk';
  if (item.status === 'done' && !kinds.length) return 'done';
  if (item.aggregate.working) return 'run';
  return kinds.includes('wait') ? 'wait' : 'idle';
}
// The card title: the root's name by the app title rules (model.js rootTitle, D3).
const cardTitle = item => item.name;
// Line 2: one tile per active direct subagent, ✓n for finished ones, then the counts.
const strip = item => directTiles(item.root, item.nativeChildren?.length ?? 0);
const swept = item => cardState(item) === 'run' && Boolean(item.aggregate.sweep);
// The running sweep lights a stretch of the card's own bottom border (SPEC 4.7). It is activity,
// not a measure of progress: a 28% window travels corner to corner across the full border-box
// width. Through that window shows a copy of the card's bottom outline, pinned to the card, so the
// light bends round the corners and needs no clip at the card's ends. gpui paints an element's
// border over its children, so the sweep is the card's sibling, laid over it by the parent. The
// outline is twice the radius tall, or gpui would clamp the radius; the window shows its lower half.
function sweep(on, now) {
  if (!on) return [];
  const p = (now % 2400) / 2400, eased = p < .5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
  const at = -.28 + eased * 1.28, percent = value => `${value * 100}%`;
  return [div().absolute().left(0).right(0).bottom(0).h(CORNER)
    .child(div().absolute().left(percent(at)).w('28%').top(0).bottom(0).overflow_hidden()
      .child(div().absolute().left(percent(-at / .28)).w(percent(1 / .28)).top(-CORNER).h(CORNER * 2)
        .border_l(1).border_r(1).border_b(1).rounded_b(CORNER).border_color(stateOf('run').hue).opacity(.85)))];
}
// A first-level row: the boxed tile. Deeper rows: the bare glyph after the 4px gap.
const stateMark = (status, boxed) => boxed ? tile(status, TILE) : glyph(status, 11);
// The steady lavender mark of a root waiting on background work (rail and native child rows).
const waitingMark = () => div().ml(8).mr(1).flex_none().child(glyph('bg', 12));
export default class Panorama extends View {
  init(_props, cx) {
    this.folds = new Map(); this.hovered = null; this.held = null;
    this.changed = {}; this.scope = 'space'; this.resume = false;
    // The scope and the resume toggle persist in the retained panorama-spaces store (no shared
    // key); that mod writes them on our command. A choice made before the read lands wins.
    cx.spawn(async cx => { try { const text = await readTextFile(`${appHome()}/data/panorama-spaces/store.json`); if (text) { if (!this.scopeSet) this.scope = storedScope(text); if (!this.resumeSet) this.resume = storedResume(text); cx.notify(); } } catch {} });
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
    this.peekCaller.show({ root: item.root, agent: row.agent }, row.key, false, cx);
  }
  setScope(scope, cx) {
    this.scope = scope; this.scopeSet = true; cx.notify();
    run(`panorama-spaces.agents-scope-${scope}`);
  }
  // Optional agent resume: a stopgap until herdr-app does this itself (herdr-core#13).
  setResume(on, cx) {
    this.resume = on; this.resumeSet = true; cx.notify();
    run(`panorama-spaces.agent-resume-${on ? 'on' : 'off'}`);
  }
  // The last core loss: one line, opening to a row per terminal with its reason and command.
  resumeRows(t, append) {
    const summary = resumeSummary(this.feed?.feed?.resume);
    if (!summary || this.resumeHidden === summary.id) return;
    append(h_flex().id('resume-result').h(24).pl(6).pr(4).gap(6).flex_none().items_center().rounded(6).text_size(11.5).cursor_pointer().hover(el => el.bg(DOCK.hover))
      .on_click((_event, cx) => { this.resumeOpen = !this.resumeOpen; cx.notify(); })
      .child(div().flex_none().text_color(t.dim).child('Resume'))
      .child(div().flex_1().min_w(0).truncate().text_color(t.foreground).child(summary.text))
      .when(!summary.open, el => el.child(div().id('resume-hide').px(4).rounded(4).text_color(t.dim).hover(el => el.text_color(t.foreground))
        .on_click((event, cx) => { cx.stop_propagation(); this.resumeHidden = summary.id; cx.notify(); }).child('×'))), 24);
    if (this.resumeOpen) for (const row of summary.rows) {
      const detail = [row.reason, row.word !== 'resumed' ? row.command : ''].filter(Boolean).join('\n');
      append(h_flex().id(row.key).h(ROW).pl(14).pr(8).gap(6).flex_none().items_center().text_size(11.5).when(Boolean(detail), el => el.tooltip(detail))
        .child(div().flex_none().text_color(t.muted).child(`${row.terminal} ${row.harness}`))
        .child(div().flex_1().min_w(0).truncate().text_color(t.dim).child(row.reason ? `${row.word}: ${row.reason}` : row.word)), ROW);
    }
  }
  newAgent() { openOverlay({ mod: 'new-menu', kind: 'popup', width: 250, height: 512 }); }
  sortMenu(t) {
    const options = [{ id: 'priority', label: 'Priority', icon: 'clock' }, { id: 'grouped', label: 'Grouped by space', icon: 'layers' }];
    return v_flex().w_full().p(5).bg(t.popover).text_size(12.8)
      .child(div().px(8).pt(4).pb(6).text_size(11).text_color(t.dim).child('Sort agents'))
      .children(options.map(option => h_flex().id(`sort-${option.id}`).h(31).px(8).gap(9).rounded(7).items_center().text_color(t.foreground).hover(el => el.bg(t.hover))
        .on_click(() => { setShared('agents.sort', option.id); closeOverlay(props().overlay); })
        .child(svg(`mods/workspace-chrome/${option.icon}.svg`).size(14)).child(div().flex_1().child(option.label))
        .when((shared('agents.sort') ?? 'priority') === option.id, el => el.child(svg('mods/workspace-chrome/check.svg').size(14)))))
      .child(div().px(8).pt(9).pb(6).text_size(11).text_color(t.dim).child('After a core restart'))
      .child(h_flex().id('resume-toggle').h(31).px(8).gap(9).rounded(7).items_center().text_color(t.foreground).hover(el => el.bg(t.hover))
        .tooltip('Types claude --resume or codex resume into idle shells that lost a live session. Only until herdr-app does this itself.')
        .on_click((_event, cx) => this.setResume(!this.resume, cx))
        .child(div().flex_1().child('Resume agents'))
        .when(this.resume, el => el.child(svg('mods/workspace-chrome/check.svg').size(14))));
  }
  foldControl(key, signature, open, count, color, cx) {
    return h_flex().id(`fold-${key}`).h(18).ml(6).px(3).gap(3).items_center().flex_none().text_size(11).text_color(color).rounded(4).hover(el => el.bg(theme().hover))
      .on_click((event, cx) => { cx.stop_propagation(); this.toggle(key, signature, open, cx); })
      .child(svg(`mods/workspace-chrome/chevron-${open ? 'down' : 'right'}.svg`).size(11))
      .when(!open, el => el.child(String(count)));
  }
  // V11 (as the TUI dock): only the docked entry opens into its list by default.
  // A stale feed-only root never opens: its rows would replay old feed states as fresh.
  rootOpen(item) { return !item.stale && isOpen(this.folds, item.key, rootSignature(item), item.focused); }
  rootRow(item, index, compact, now, cx, joined = false, scope = 'space') {
    const t = theme(), sig = rootSignature(item), open = this.rootOpen(item);
    const count = item.stale ? 0 : (item.root.agents?.length ?? 0) + (item.root.hidden_finished > 0 ? 1 : 0) + item.nativeChildren.length;
    if (compact) return h_flex().id(`root-${item.key}`).h(26).px(8).flex_none().items_center().gap(4).hover(el => el.bg(t.hover))
      .when(Boolean(item.terminal), el => el.on_click(() => run(rootTarget(item).name, rootTarget(item).args)))
      .child(div().flex_1().min_w(0).text_right().text_color(t.muted).child(String(item.stock?.tab_index ?? (index >= 0 ? index + 1 : ""))))
      .child(item.status === 'bg' ? waitingMark() : mark(t, item.aggregate.kinds.length ? 'blk' : item.status, item.aggregate.questions, true));
    // One graph-style card per root. Line 1: tab number, harness icon, title, ?N (when not
    // already the wait badge), state badge. Line 2: direct-subagent tiles and counts.
    const pic = item.program ?? iconProgram(item.root.harness), state = cardState(item), hue = stateOf(state).hue;
    const hovered = this.hovered === item.key, docked = item.focused, colors = cardColors(state, docked);
    const fill = hovered && !docked ? mix(hue, 0x0e / 255, DOCK.hover) : colors.fill;
    const questions = item.aggregate.questions, line = item.stale ? { tiles: [], done: 0, active: 0, live: 0 } : strip(item), quiet = docked && open && count > 0;
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
        .child(item.stale ? '' : [cardPlace(item, scope), `${line.active} active · ${line.live} agents`].filter(Boolean).join(' · ')));
    // An open card continues into its rows (inCard), so its bottom stays square and open.
    return div().relative().pt(4).h(52).flex_none().child(div().id(`root-${item.key}`).relative().h(48).border_color(colors.border).bg(hue)
      .when(joined, el => el.rounded_t(8).border_t(1).border_l(1).border_r(1)).when(!joined, el => el.rounded(8).border(1))
      .on_hover((on, cx) => { this.hovered = on ? item.key : (this.hovered === item.key ? null : this.hovered); cx.notify(); })
      .when(Boolean(item.terminal), el => el.cursor_pointer().on_click(() => run(rootTarget(item).name, rootTarget(item).args)))
      .child(v_flex().absolute().left(3).right(0).top(0).bottom(0).when(joined, el => el.rounded_tr(7)).when(!joined, el => el.rounded_r(7)).bg(fill).child(head).child(tail))
      .when(joined, el => el.child(div().absolute().left(TRUNK).top(23).bottom(0).w(1.5).bg(t.input))))
      .children(sweep(!joined && swept(item), now));
  }
  // A row inside its root's card: the card's state edge, tint and border continue down to
  // the last row, which closes the card (V11 B: the docked entry holds its list) and so carries the sweep.
  inCard(item, element, last, now) {
    const state = cardState(item), colors = cardColors(state, item.focused);
    const row = div().relative().h(ROW + (last ? 5 : 0)).flex_none().bg(stateOf(state).hue).border_l(1).border_r(1).border_color(colors.border)
      .when(last, el => el.border_b(1).rounded_b(8))
      .child(div().absolute().left(3).right(0).top(0).bottom(0).bg(colors.fill).when(last, el => el.rounded_br(7)).child(element));
    return last ? div().relative().flex_none().child(row).children(sweep(swept(item), now)) : row;
  }
  // V11: one row component for every agent (own glyph, title, a run of states) and the
  // root's `N finished · titles` fold. The tree trunk starts under the card's icon.
  // A first-level row boxes its state like line 2. The tile replaces the 4px gap, so the
  // connector runs into it, and -1 centres it on its children's line (x=7 of the next segment);
  // the title stays where the 11px glyph left it.
  detailRow(item, row, now, cx) {
    const t = theme(), agent = row.agent, boxed = row.lines.length === 1;
    const base = h_flex().id(row.key).relative().pl(29).pr(10).h(ROW).flex_none().rounded(4).items_center().gap(0).text_size(12.5).hover(el => el.bg(t.hover)).when(this.hovered === row.key, el => el.bg(t.hover)).when(this.peekCaller?.rowKey === row.key, el => el.bg(t.raised)).cursor_pointer().children(lines(t, row.lines, ROW)).when(!boxed, el => el.child(div().w(4).flex_none()));
    if (row.kind === 'summary') {
      const label = [row.count ? `${row.count} finished` : null, row.names.join(', ') || null, row.older ? `+${row.older} older` : null].filter(Boolean).join(' · ');
      return base.when(row.count > 0, el => el.on_click((_event, cx) => this.toggle(row.key, row.signature, row.open, cx)))
        .child(div().mr(7).flex_none().when(boxed, el => el.ml(-1)).child(stateMark('done', boxed))).child(div().flex_1().min_w(0).truncate().text_color(SURFACE.dim).child(label));
    }
    const on = this.hovered === row.key;
    return base.on_click((_event, cx) => this.peek(item, row, cx)).on_hover((hovered, cx) => { this.hovered = hovered ? row.key : null; cx.notify(); })
      .child(div().id(`icon-${row.key}`).mr(7).flex_none().when(boxed, el => el.ml(-1)).when(row.children > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(row.key, row.signature, row.open, cx); })).child(stateMark(agent.status, boxed)))
      .child(div().flex_1().min_w(0).truncate().text_color(needs(agent.status) ? DOCK.hi : DOCK.fg).child(agentTitle(agent)))
      .when(on, el => el.child(div().h(18).ml(6).px(6).rounded(5).flex_none().bg(t.raised).text_size(10.5).font_medium().text_color(t.foreground).child('Peek')))
      .child(div().ml(6).flex_none().child(glyphRun(row.run.statuses, 6, row.run.finished)));
  }
  render(cx) {
    const t = theme(); if (props()?.sortMenu) return this.sortMenu(t);
    this.feed = consumerFeed(this); this.peekCaller.sync();
    const now = Date.now(), compact = narrow(), raw = agents(), all = terminals(), ps = panes(), ss = spaces();
    const current = raw.find(agent => agent.focused);
    if (!current || current.address !== this.held?.address || current.status !== 'idle') this.held = current ? { address: current.address, status: current.status } : null;
    const stock = raw.map(agent => agent.address === this.held?.address && this.held.status === 'done' && agent.status === 'idle' ? { ...agent, status: 'done' } : agent);
    for (const agent of stock) if (this.changed[agent.address]?.status !== agent.status) { this.changed[agent.address] = { status: agent.status, at: now }; }
    for (const root of this.feed?.feed?.roots ?? []) {
      const key = JSON.stringify(['root', root.host, root.host === 'herdr-core' && root.terminal !== null ? root.terminal : root.pane_id]);
      if (this.changed[key]?.status !== root.herdr_status) { this.changed[key] = { status: root.herdr_status, at: now }; }
    }
    const everything = joinRoots(this.feed?.feed, stock, all, ps, ss, { changes: this.changed, aged: this.feed?.aged });
    setAgentOrder(everything.filter(item => item.stock).map(item => item.stock.address));
    // V10: the active space's roots, plus roots known only from the feed (listed());
    // All spaces lists every space's roots in the same sections.
    const active = ss.find(space => space.active)?.id ?? null, scope = this.scope, everySpace = scope === 'all';
    const items = listed(everything, active, Boolean(this.feed?.stale), scope);
    const totals = headerTotals(items, active, scope);
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
    if (!compact) this.resumeRows(t, append);
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
        const inner = [], open = !compact && this.rootOpen(item);
        if (!compact) {
          for (const row of rows(item.root, this.folds, open)) inner.push(this.detailRow(item, row, now, cx));
          if (open && item.stock) for (const child of item.nativeChildren ?? []) {
            const childSig = rootSignature(child), childOpen = isOpen(this.folds, child.key, childSig, child.aggregate.priority === 4 || child.aggregate.priority === 2);
            const count = child.root.agents?.length ?? 0, segments = Array.from({ length: child.depth + 1 }, (_, i) => i === child.depth ? '└' : '│');
            inner.push(h_flex().id(`terminal-${child.terminal}`).relative().h(ROW).pl(29).pr(10).flex_none().rounded(4).items_center().text_size(12.5).gap(0).hover(el => el.bg(t.hover)).cursor_pointer()
              .on_click(() => run(rootTarget(child).name, rootTarget(child).args)).children(lines(t, segments, ROW)).child(div().w(4).flex_none())
              .child(div().id(`native-fold-${child.terminal}`).mr(7).when(count > 0, el => el.on_click((event, cx) => { cx.stop_propagation(); this.toggle(child.key, childSig, childOpen, cx); })).child(icon(child.program, 14, t.foreground)))
              .child(div().flex_1().min_w(0).truncate().text_color(t.muted).child(child.name))
              .when(count > 0 && !childOpen, el => el.child(this.foldControl(child.key, childSig, childOpen, count, child.aggregate.kinds.length ? t.blocked : t.dim, cx)))
              .child(child.status === 'bg' ? waitingMark() : rootMarks(t, child)));
            if (childOpen) for (const row of rows(child.root, this.folds, true)) inner.push(this.detailRow(child, { ...row, lines: [...segments.map(() => '│'), ...row.lines] }, now, cx));
          }
        }
        // The timer runs while the row that draws the sweep is on screen: the card's last.
        const moving = !compact && swept(item), last = inner.length - 1;
        append(this.rootRow(item, items.indexOf(item), compact, now, cx, inner.length > 0, scope), compact ? 26 : 52, moving && !inner.length);
        inner.forEach((element, n) => append(this.inCard(item, element, n === last, now), ROW + (n === last ? 5 : 0), moving && n === last));
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
        .child(h_flex().gap(6).items_center().child(div().text_size(11.5).font_medium().text_color(t.dim).child('Agents'))
          .child(h_flex().id('ag-scope').h(18).px(5).rounded(4).items_center().text_size(11).text_color(everySpace ? t.foreground : DOCK.number).cursor_pointer()
            .hover(el => el.bg(t.hover).text_color(t.foreground))
            .on_click((_event, cx) => this.setScope(everySpace ? 'space' : 'all', cx))
            .child(everySpace ? 'All spaces' : 'This space')))
        .child(h_flex().gap(8).items_center()
          .child(h_flex().mr(2).gap(7).items_center().text_size(11).font_bold()
            .when(totals.wait > 0, el => el.child(div().text_color(DOCK.need).child(`?${totals.wait}`)))
            .when(totals.blk > 0, el => el.child(div().text_color(DOCK.need).child(`!${totals.blk}`)))
            .when(totals.fail > 0, el => el.child(div().text_color(DOCK.fail).child(`×${totals.fail}`))))
          .when(errors.length || this.feed?.stale, el => el.child(div().text_size(11).text_color(t.dim).tooltip(errors.join('\n')).child(this.feed?.stale ? '◌' : '!')))
          .child(div().id('ag-sort').text_color(t.dim).hover(el => el.text_color(t.foreground))
            .on_click(() => openOverlay({ mod: 'panorama', kind: 'popup', width: 200, height: 196, props: { sortMenu: true } }))
            .child(svg(`mods/workspace-chrome/${grouped ? 'layers' : 'clock'}.svg`).size(14))))))
      .child(viewport(this, rendered, compact))
      .when(compact, el => el.child(h_flex().h(28).items_center().justify_center().child(div().id('expand-sidebar').text_size(13).text_color(t.dim).on_click(() => { const width = shared('sidebar.width') ?? 320; setShared('sidebar.collapsed', false); run('layout.resize', { id: 'sidebar', px: width }); }).child('»'))));
  }
}
