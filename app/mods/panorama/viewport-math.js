// The scroll position belongs to this viewport, never inferred from native
// scrolling. Extents are the same fixed row heights used by its elements.
export const logical = (physical, zoom) => physical / (Number.isFinite(zoom) && zoom > 0 ? zoom : 1);
export const clampOffset = (offset, total, height) => Math.max(0, Math.min(Number.isFinite(offset) ? offset : 0, Math.max(0, total - height)));
export function visibleRows(entries, offset, height) {
  const result = []; let top = 0;
  for (let index = 0; index < entries.length; index++) {
    const bottom = top + entries[index].height;
    if (height > 0 && bottom > offset && top < offset + height) result.push({ index, top, bottom, ...entries[index] });
    top = bottom;
  }
  return result;
}

// Herdr app0.1.12's pinned gpui-base ThumbGeometry::new (blob456addb4):
// the 48px minimum is the full logical thumb; paint excludes both4px insets.
// These base-layer extents are physical pixels, independent of script zoom.
export function thumbGeometry(total, height, zoom) {
  const length = Math.min(height, Math.max(logical(48, zoom), total ? height * height / total : height));
  const inset = Math.min(logical(4, zoom), length / 2);
  return { inset, width: logical(6, zoom), hit: logical(16, zoom), thumb: length - inset * 2 };
}
