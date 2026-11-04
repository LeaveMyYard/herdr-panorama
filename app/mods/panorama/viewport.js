import { div } from 'gpui-kit';
import { v_flex, SliderState, Slider, SliderTrack, SliderIndicator, SliderThumb } from 'gpui-base';
import { settings, theme } from 'herdr';
import { logical, clampOffset, visibleRows, thumbGeometry } from './viewport-math.js';
const zoom = () => { const value = Number(settings().zoom); return Number.isFinite(value) && value > 0 ? value : 1; };
export function initViewport(view, cx) {
  view.scrollFocus = cx.focus_handle();
  view.scrollOffset = 0; view.viewportPhysical = { width: 0, height: 0 }; view.contentHeight = 0;
  view.scrollState = SliderState.new({ min: 0, max: 1, step: .000001, value: 1 });
  view.scrollState.on('change', (value, cx) => {
    const height = logical(view.viewportPhysical.height, zoom());
    view.scrollOffset = clampOffset((1 - Number(value)) * Math.max(0, view.contentHeight - height), view.contentHeight, height);
    cx.notify();
  });
}
function synchronize(view, cx) {
  const height = logical(view.viewportPhysical.height, zoom()), max = Math.max(0, view.contentHeight - height);
  view.scrollOffset = clampOffset(view.scrollOffset, view.contentHeight, height);
  const value = max ? 1 - view.scrollOffset / max : 1;
  if (Math.abs(Number(view.scrollState.value()) - value) > .000001) view.scrollState.set_value(value);
  cx.notify();
}
export function wheelViewport(view, event, cx) {
  if (!Number.isFinite(event.delta.y)) return;
  const height = logical(view.viewportPhysical.height, zoom());
  view.scrollOffset = clampOffset(view.scrollOffset - logical(event.delta.y, zoom()), view.contentHeight, height);
  synchronize(view, cx); cx.stop_propagation();
}
export function keyViewport(view, event, cx) {
  if (!view.scrollFocus.is_focused() || event.modifiers.control || event.modifiers.alt || event.modifiers.platform) return;
  if (event.key === 'n' && !event.modifiers.shift && view.newAgent) { view.newAgent(); cx.stop_propagation(); return; }
  const height = logical(view.viewportPhysical.height, zoom());
  const max = Math.max(0, view.contentHeight - height);
  const next = { up: view.scrollOffset - 26, down: view.scrollOffset + 26,
    home: 0, end: max, pageup: view.scrollOffset - height * .9, pagedown: view.scrollOffset + height * .9 }[event.key];
  if (next === undefined) return;
  view.scrollOffset = clampOffset(next, view.contentHeight, height); synchronize(view, cx); cx.stop_propagation();
}
export function viewport(view, entries, compact) {
  const t = theme(), height = logical(view.viewportPhysical.height, zoom());
  view.contentHeight = entries.reduce((sum, row) => sum + row.height, 0);
  view.scrollOffset = clampOffset(view.scrollOffset, view.contentHeight, height);
  const visible = visibleRows(entries, view.scrollOffset, height);
  view.animate = visible.some(row => row.animated);
  const bar = thumbGeometry(view.contentHeight, height, zoom()), thumb = bar.thumb;
  const overflow = view.contentHeight > height && height > 0;
  return div().id('ag-viewport').track_focus(view.scrollFocus).role('group').accessibility_label('Agents').relative().flex_1().min_h(0).overflow_hidden()
    .on_size_change((size, cx) => { view.viewportPhysical = size; synchronize(view, cx); })
    .on_scroll_wheel((event, cx) => view.wheel(event, cx))
    .on_key_down((event, cx) => keyViewport(view, event, cx))
    .child(div().id('ag-extent').absolute().left(0).right(0).top(-view.scrollOffset).h(view.contentHeight)
      .on_size_change((_size, cx) => synchronize(view, cx))
      .child(v_flex().absolute().left(compact ? 0 : 8).right(compact ? 0 : 8).top(visible[0]?.top ?? 0)
        .children(visible.map(row => row.element))))
    .when(overflow, el => el.child(div().id('ag-scroll-control').absolute().right(0).top(0).bottom(0).w(bar.hit)
      .on_mouse_down('left', () => view.scrollFocus.focus()).on_mouse_up('left', () => view.scrollFocus.focus())
      .child(Slider.new(view.scrollState).accessibility_label('Agents').axis('vertical').size_full()
      .child(SliderTrack.new(view.scrollState).axis('vertical').relative().size_full()
        .child(SliderIndicator.new(view.scrollState).axis('vertical').absolute().top(bar.inset + thumb / 2).bottom(bar.inset + thumb / 2).right(bar.inset).w(bar.width)
          .child(SliderThumb.new(view.scrollState).id('ag-scroll-thumb').axis('vertical').w(bar.width).h(thumb).mb(-thumb / 2).bg(t.foreground + '59')))))));
}
