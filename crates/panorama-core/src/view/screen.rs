//! The cell grid a view renders into (Python `render.Screen`), with its click targets and
//! scroll regions.
//!
//! One cell per character, as Python counts characters: every glyph panorama draws is one
//! cell wide. The binary converts a screen to a ratatui `Buffer` at the terminal's edge.
//! Rows are made on first write, so a tall column canvas costs only what is drawn on it.

use super::colour::Rgb;
use super::palette::FG;
use super::seg::Seg;
use super::target::Hit;

/// One cell: a character, its colour, its background (`None`: the terminal's, never set) and
/// its weight; and whether its colour or background moves with the animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub fg_moves: bool,
    pub bg_moves: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: FG,
            bg: None,
            bold: false,
            fg_moves: false,
            bg_moves: false,
        }
    }
}

/// A scrollable column as painted: the wheel scrolls `column` over `x0..x1`, `y0..y1`, and
/// `offset` is where the render left it (the caller keeps it for the next render).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollRegion {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
    pub column: usize,
    pub offset: i32,
}

/// A grid of `w` × `h` cells (its size is fixed when it is made).
#[derive(Clone, Debug)]
pub struct Screen {
    w: i32,
    h: i32,
    rows: Vec<Option<Vec<Cell>>>,
    /// Click targets in the order registered; a later one wins over an earlier one.
    pub targets: Vec<Hit>,
    /// The scrollable columns.
    pub columns: Vec<ScrollRegion>,
}

impl Screen {
    /// A blank grid.
    pub fn new(w: i32, h: i32) -> Self {
        let (w, h) = (w.max(0), h.max(0));
        Self {
            w,
            h,
            rows: vec![None; h as usize],
            targets: vec![],
            columns: vec![],
        }
    }

    /// Its width in cells.
    pub fn w(&self) -> i32 {
        self.w
    }

    /// Its height in cells.
    pub fn h(&self) -> i32 {
        self.h
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        (0..self.w).contains(&x) && (0..self.h).contains(&y)
    }

    /// The cell at `(x, y)` (a blank one where nothing was drawn or outside).
    pub fn cell(&self, x: i32, y: i32) -> Cell {
        if !self.inside(x, y) {
            return Cell::default();
        }
        self.rows[y as usize]
            .as_ref()
            .map_or_else(Cell::default, |r| r[x as usize])
    }

    /// The cell at `(x, y)` to change, `None` outside.
    pub fn cell_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if !self.inside(x, y) {
            return None;
        }
        let w = self.w as usize;
        let row = self.rows[y as usize].get_or_insert_with(|| vec![Cell::default(); w]);
        Some(&mut row[x as usize])
    }

    /// Whether row `y` has been drawn on.
    pub fn drawn(&self, y: i32) -> bool {
        (0..self.h).contains(&y) && self.rows[y as usize].is_some()
    }

    /// Each drawn cell, to change in place.
    pub fn cells_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.rows.iter_mut().flatten().flatten()
    }

    /// `text` from `(x, y)`, one cell per character; the background is kept when `bg` is
    /// `None`. The x after it (Python `put`).
    pub fn put(&mut self, x: i32, y: i32, text: &str, fg: Rgb, bg: Option<Rgb>, bold: bool) -> i32 {
        let mut seg = Seg::new(text, fg).bold_if(bold);
        seg.bg = bg;
        self.put_seg(x, y, &seg)
    }

    /// A segment from `(x, y)`, one cell per character; the x after it.
    pub fn put_seg(&mut self, x: i32, y: i32, s: &Seg) -> i32 {
        let mut x = x;
        for ch in s.text.chars() {
            if let Some(c) = self.cell_mut(x, y) {
                c.ch = ch;
                c.fg = s.fg;
                c.fg_moves = s.fg_moves;
                c.bold = s.bold;
                if s.bg.is_some() {
                    c.bg = s.bg;
                    c.bg_moves = s.bg_moves;
                }
            }
            x += 1;
        }
        x
    }

    /// Segments one after another from `(x, y)`; the x after them (Python `P`).
    pub fn segs(&mut self, x: i32, y: i32, segs: &[Seg]) -> i32 {
        segs.iter().fold(x, |x, s| self.put_seg(x, y, s))
    }

    /// Segments ending at `xr` (Python `_rp`).
    pub fn segs_right(&mut self, xr: i32, y: i32, segs: &[Seg]) {
        self.segs(xr - super::seg::width(segs), y, segs);
    }

    /// The background of a `w` × `h` box at `(x, y)`.
    pub fn fill_bg(&mut self, x: i32, y: i32, w: i32, h: i32, col: Rgb) {
        self.fill_bg_moving(x, y, w, h, col, false);
    }

    /// The same, with a background that moves with the animation when `moves`.
    pub fn fill_bg_moving(&mut self, x: i32, y: i32, w: i32, h: i32, col: Rgb, moves: bool) {
        for j in y..y + h {
            for i in x..x + w {
                if let Some(c) = self.cell_mut(i, j) {
                    c.bg = Some(col);
                    c.bg_moves = moves;
                }
            }
        }
    }

    /// Rows `sy..sy + h` of `src` copied to `(dx, dy)`: drawn cells only (a character or a
    /// background), and the targets on those rows (Python `blit`).
    pub fn blit(&mut self, src: &Screen, sy: i32, dx: i32, dy: i32, h: i32) {
        for j in 0..h {
            if !src.drawn(sy + j) || !(0..self.h).contains(&(dy + j)) {
                continue;
            }
            for i in 0..src.w {
                let c = src.cell(i, sy + j);
                if (c.ch != ' ' || c.bg.is_some())
                    && let Some(d) = self.cell_mut(dx + i, dy + j)
                {
                    *d = c;
                }
            }
        }
        for t in &src.targets {
            if (sy..sy + h).contains(&t.y) && (0..self.h).contains(&(dy + t.y - sy)) {
                self.target(dx + t.x0, dx + t.x1, dy + t.y - sy, t.target.clone());
            }
        }
    }

    /// The characters, a line per row, trailing blanks cut (Python `text`).
    pub fn text(&self) -> String {
        (0..self.h)
            .map(|y| {
                (0..self.w)
                    .map(|x| self.cell(x, y).ch)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

mod hits;

#[cfg(test)]
mod tests;
