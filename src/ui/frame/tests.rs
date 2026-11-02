use panorama_core::view::harness::{CLAUDE, CODEX};
use unicode_width::UnicodeWidthChar;

#[test]
fn vendor_marks_match_the_terminals_single_cell_geometry() {
    for glyph in [CLAUDE, CODEX] {
        assert_eq!(glyph.width(), Some(1));
    }
}
