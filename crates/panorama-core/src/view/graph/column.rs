//! A column drawn on its own tall canvas, then cropped around the selection (Python
//! `render.Column`).

use crate::view::screen::Screen;

/// Rows past this are cut (Python `TALL`).
pub const TALL: i32 = 400;

/// A column as drawn: its canvas, the row each card's link joins, the selected card's rows
/// and link row, and the height of what was drawn.
#[derive(Clone, Debug)]
pub struct Column {
    pub canvas: Screen,
    pub anchors: Vec<i32>,
    pub sel: Option<(i32, i32)>,
    pub sel_anchor: Option<i32>,
    pub height: i32,
}

impl Column {
    /// An empty column `w` cells wide.
    pub fn new(w: i32) -> Self {
        Self {
            canvas: Screen::new(w, TALL),
            anchors: vec![],
            sel: None,
            sel_anchor: None,
            height: 0,
        }
    }

    /// A card drawn at row `y`, `h` rows: its link joins its first line; `is_sel` makes it the
    /// selection.
    pub fn card(&mut self, y: i32, h: i32, is_sel: bool) {
        self.anchors.push(y + 1);
        if is_sel {
            self.sel = Some((y, y + h));
            self.sel_anchor = Some(y + 1);
        }
    }
}

/// Where a card goes in its column, and how it is selected (`focus`: the keyboard is in
/// its column, on it).
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub is_sel: bool,
    pub focus: bool,
}
