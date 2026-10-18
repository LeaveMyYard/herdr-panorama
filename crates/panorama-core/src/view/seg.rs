//! A run of text in one style (Python's segments: a plain string or `(text, fg, bold, bg)`).

use super::colour::Rgb;
use super::palette::FG;

/// Text in one colour, bold or not, over a background (`None`: keep the cell's).
///
/// `fg_moves` / `bg_moves` mark a colour that changes with the animation time (Python
/// draws those as `Anim`): the theme leaves such colours as drawn (`dock::look`).
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub text: String,
    pub fg: Rgb,
    pub bold: bool,
    pub bg: Option<Rgb>,
    pub fg_moves: bool,
    pub bg_moves: bool,
}

impl Seg {
    /// `text` in `fg`.
    pub fn new(text: impl Into<String>, fg: Rgb) -> Self {
        Self {
            text: text.into(),
            fg,
            bold: false,
            bg: None,
            fg_moves: false,
            bg_moves: false,
        }
    }

    /// `text` in the text colour (a bare string segment).
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, FG)
    }

    /// Bold when `on`.
    pub fn bold_if(mut self, on: bool) -> Self {
        self.bold = on;
        self
    }

    /// Bold.
    pub fn bold(self) -> Self {
        self.bold_if(true)
    }

    /// Over `bg`.
    pub fn on(mut self, bg: Rgb) -> Self {
        self.bg = Some(bg);
        self
    }

    /// Its colour moves with the animation when `on`.
    pub fn fg_moving(mut self, on: bool) -> Self {
        self.fg_moves = on;
        self
    }

    /// Its background moves with the animation when `on`.
    pub fn bg_moving(mut self, on: bool) -> Self {
        self.bg_moves = on;
        self
    }

    /// Its width in cells: one per character, as Python counts.
    pub fn len(&self) -> i32 {
        self.text.chars().count() as i32
    }

    /// Whether it has no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// The width of segments in cells (Python `_seglen`).
pub fn width(segs: &[Seg]) -> i32 {
    segs.iter().map(Seg::len).sum()
}
