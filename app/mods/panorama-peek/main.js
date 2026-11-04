import { View, div, svg } from 'gpui-kit';
import { v_flex, h_flex, Editor, EditorState } from 'gpui-base';
import { agents, terminals, panes, spaces, props, theme, settings, window, run, hc, closeOverlay } from 'herdr';
import { startConsumer, consumerFeed, closeConsumer } from './feed-client.js';
import { harnessOf, iconProgram, joinRoots } from './model.js';
import { agentTitle } from './tree.js';
import { rootKey, requireProbe } from './contract.js';
import { icon } from './marks.js';
import { badge, stateOf } from './state.js';
import { subject, transcriptScope, focusTarget } from './subject.js';
import { Pages } from './paging.js';
import { peekCommand, guard } from './actions.js';
import { metadata } from './metadata.js';
function lighten(color) {
  if (!/^#[0-9a-f]{6}$/i.test(color)) return color;
  return '#' + [1, 3, 5].map(i => Math.round(parseInt(color.slice(i, i + 2), 16) * .6 + 255 * .4).toString(16).padStart(2, '0')).join('');
}
function button(label, key, enabled, solid, action, t) {
  return h_flex().h(28).px(10).gap(5).rounded(7).items_center().justify_center().flex_none().text_size(12)
    .bg(solid ? t.solid : t.raised).text_color(solid ? t.solid_foreground : t.foreground).opacity(enabled ? 1 : .4)
    .when(enabled, el => el.cursor_pointer().on_click((_event, cx) => action(cx)))
    .child(div().when(solid, el => el.font_medium()).child(label))
    .child(div().text_size(11).font_family('JetBrains Mono').opacity(.6).child(key));
}
export default class PanoramaPeek extends View {
  init(initial, cx) {
    this.intent = initial ?? props() ?? {}; this.viewer = this.intent.action === 'transcript';
    this.closed = false; this.editor = null; this.requested = null; this.shown = null; this.busy = false; this.reading = false;
    this.handle = cx.focus_handle(); this.handle.focus();
    this.client = startConsumer(this, cx); this.newPages();
  }
  value() { return subject(consumerFeed(this), this.client, this.intent); }
  scope() { return transcriptScope(this.value(), window()?.platform); }
  current(scope) { return !this.closed && this.scope()?.key === scope?.key; }
  async validate() {
    await requireProbe(args => this.client.callPrivate(args));
  }
  newPages() {
    this.pages?.close();
    this.pages = new Pages(args => {
      const scope = this.scope();
      return guard(scope, current => this.current(current), () => this.validate(), () => hc(args));
    }, scope => this.current(scope));
  }
  releaseEditor() { this.editor?.release(); this.editor = null; }
  load(direction, cx) {
    const scope = this.scope(); if (!scope || this.closed || this.reading) return false;
    // This view-wide gate survives page resets while an old native read settles.
    const pages = this.pages; this.reading = true;
    if (pages.page?.scope !== scope.key) this.releaseEditor();
    cx.spawn(async cx => {
      try {
        const read = direction === 'next' ? pages.next(scope) : direction === 'previous' ? pages.previous(scope) : pages.load(scope, pages.page?.scope === scope.key ? pages.page.first : 1);
        const changed = await read;
        if (this.closed || pages !== this.pages || !this.current(scope)) return;
        if (changed) {
          this.releaseEditor();
          this.editor = EditorState.new({ readonly: true, value: pages.page.text, language: 'json', soft_wrap: false,
            font_family: settings().font_family, font_size: 12 });
        }
      } finally { this.reading = false; if (!this.closed) cx.notify(); }
    });
    return true;
  }
  target(value) { return focusTarget(value, this.intent, { feed: consumerFeed(this).feed, agents: agents(), terminals: terminals(), panes: panes(), spaces: spaces() }); }
  open(cx) {
    const value = this.value(); if (!value || this.busy) return;
    this.busy = true;
    cx.spawn(async cx => {
      try {
        await this.validate();
        if (!this.closed && this.value()?.key === value.key) {
          const target = this.target(this.value());
          if (target) run(target.name, target.args);
        }
      } catch (error) { if (!this.closed) this.actionError = String(error); }
      finally { this.busy = false; if (!this.closed) cx.notify(); }
    });
  }
  transcript(cx) {
    const scope = this.scope(), command = peekCommand(this.intent, 'transcript');
    if (!scope || !command || this.busy) return;
    this.busy = true;
    cx.spawn(async cx => {
      try { await this.validate(); if (this.current(scope)) run(command); }
      catch (error) { if (!this.closed && this.current(scope)) this.actionError = String(error); }
      finally { this.busy = false; if (!this.closed) cx.notify(); }
    });
  }
  close(cx) {
    this.closed = true; this.pages.close(); this.releaseEditor(); this.handle.release(); closeConsumer(this, cx);
    const command = peekCommand(this.intent, 'closed'); if (command) run(command);
  }
  key(event, cx) {
    if (event.modifiers.control || event.modifiers.platform || event.modifiers.alt) return;
    if (event.key === 'enter' && !this.viewer) { this.open(cx); cx.stop_propagation(); }
    else if (event.key === 't') { this.viewer ? this.load('refresh', cx) : this.transcript(cx); cx.stop_propagation(); }
    else if (this.viewer && ['left', 'right'].includes(event.key)) { this.load(event.key === 'left' ? 'previous' : 'next', cx); cx.stop_propagation(); }
  }
  render(cx) {
    const t = theme(), value = this.value(), scope = this.scope();
    const shown = this.viewer ? scope?.key ?? null : value?.key ?? null;
    if (this.shown !== shown) { this.shown = shown; this.releaseEditor(); this.newPages(); this.requested = null; }
    if (this.viewer && scope && this.requested !== scope.key && this.load('initial', cx)) this.requested = scope.key;
    const frame = v_flex().id('panorama-peek').w_full().min_h(0).p(12).gap(9).bg(t.popover).border(1).border_color('#ffffff1f').rounded(10)
      .text_size(12.5).track_focus(this.handle).on_key_down((event, cx) => this.key(event, cx));
    if (this.viewer) {
      const page = this.pages.page, enabled = Boolean(scope) && !this.reading && !this.pages.pending;
      return frame.size_full().child(h_flex().items_center().gap(8).child(div().font_semibold().child('Transcript'))
        .child(div().flex_1().text_size(11.5).text_color(t.dim).child(page ? `${page.first}–${page.last} / ${page.lines}` : ''))
        .child(button('←', '', enabled && this.pages.history.length > 0, false, cx => this.load('previous', cx), t))
        .child(button('→', '', enabled && page?.last < page?.lines, false, cx => this.load('next', cx), t))
        .child(div().cursor_pointer().on_click(() => closeOverlay(props().overlay)).child(svg('mods/workspace-chrome/x.svg').size(12).text_color(t.dim))))
        .when(Boolean(this.pages.error), el => el.child(div().text_size(11.5).text_color(t.blocked).child(this.pages.error.raw)))
        .child(this.editor ? Editor.new(this.editor).flex_1().min_h(0) : div().flex_1());
    }
    const agent = value?.agent, root = value?.root, harness = value ? harnessOf(agent, root) : 'unknown';
    const item = value ? joinRoots(consumerFeed(this).feed, agents(), terminals(), panes(), spaces()).flatMap(root => [root, ...root.nativeChildren]).find(item => rootKey(item.root) === rootKey(root)) : null;
    const space = item?.space ? spaces().find(space => space.id === item.space) : null;
    // V11: titled by the agent's name (else its root's), with role · model beside it.
    const rootName = item?.name || root?.title || root?.tab_label || root?.pane_id || '';
    const context = [space?.name, rootName].filter(Boolean).join(' · ');
    const heading = agent ? agentTitle(agent) : rootName, secondary = agent ? [agent.title ? agent.role : null, agent.model].filter(Boolean).join(' · ') : '';
    const error = agent?.error || this.actionError, status = agent?.status ?? root?.herdr_status;
    // V11 C: the subject's state badge, and its state edge down the card's left side.
    return frame.relative().when(Boolean(value), el => el.child(div().absolute().left(0).top(10).bottom(10).w(3).rounded(2).bg(stateOf(status).hue)))
      .child(h_flex().items_center().gap(7).child(icon(iconProgram(harness), 16, t.foreground))
      .child(div().flex_shrink(1).min_w(0).truncate().font_semibold().child(heading))
      .child(div().min_w(0).flex_1().truncate().text_color(t.dim).child(secondary))
      .when(Boolean(value), el => el.child(badge(status, undefined, { questions: agent?.question_ids.length ?? root.inbox.length, height: 20 }))))
      .child(v_flex().text_size(11.5).line_height(1.45).text_color(t.dim).child(metadata(value, harness, Date.now()))
        .when(Boolean(agent), el => el.child(`Subagent of ${context}. It has no pane of its own.`)))
      .when(Boolean(error), el => el.child(v_flex().rounded(7).px(9).py(7).bg(t.blocked + '1c').border(1).border_color(t.blocked + '40')
        .font_family(settings().font_family ?? 'monospace').text_size(11.5).line_height(1.5).text_color(lighten(t.blocked))
        .children(error.split('\n').map(line => div().child(line)))))
      .child(v_flex().gap(7).child(div().text_size(11).text_color(t.dim).child('Last message'))
        .child(div().text_size(12.5).line_height(1.5).text_color(t.muted).child(agent?.last_message ?? '')))
      .child(h_flex().gap(6).child(button('Open root pane', '⏎', Boolean(value && this.target(value)) && !this.busy, true, cx => this.open(cx), t))
        .child(button('Transcript', 't', Boolean(scope && peekCommand(this.intent, 'transcript')) && !this.busy, false, cx => this.transcript(cx), t)));
  }
}
