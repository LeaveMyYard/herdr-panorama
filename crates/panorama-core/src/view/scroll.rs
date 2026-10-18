//! A scrolled column (Python `render.scroll`): where a tall canvas is cut to show the
//! selection.

use super::state::View;

/// Column `col`'s offset: the view's, moved just enough to show the selected rows `sel`
/// (unless the wheel scrolled the column), and never past the content.
pub fn offset(
    view: &View,
    col: usize,
    sel: Option<(i32, i32)>,
    height: i32,
    content_h: i32,
) -> i32 {
    let mut off = view.offsets.get(&col).copied().unwrap_or(0);
    if let Some((top, bottom)) = sel.filter(|_| !view.wheel.contains(&col)) {
        if bottom - off > height {
            off = bottom - height;
        }
        if top - off < 0 {
            off = top;
        }
    }
    off.min((content_h - height).max(0)).max(0)
}

#[cfg(test)]
mod tests;
