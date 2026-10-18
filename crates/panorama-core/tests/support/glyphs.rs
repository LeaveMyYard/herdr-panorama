//! test_glyph_spacing.py's check: no two status glyphs in adjacent cells.

use panorama_core::view::Screen;

/// The status glyphs (the braille spinner, queued, done, failed).
pub const GLYPHS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏○✓✗";

/// Where two glyphs touch: (x, y) of the left one.
pub fn adjacent(s: &Screen) -> Vec<(i32, i32)> {
    let mut out = vec![];
    for y in 0..s.h() {
        for x in 0..s.w() - 1 {
            if GLYPHS.contains(s.cell(x, y).ch) && GLYPHS.contains(s.cell(x + 1, y).ch) {
                out.push((x, y));
            }
        }
    }
    out
}
