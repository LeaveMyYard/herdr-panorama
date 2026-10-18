use super::*;
use crate::view::palette::{BG, DIM, HI};
use crate::view::target::{Hit, Target};

#[test]
fn put_writes_one_cell_per_character_and_clips() {
    let mut s = Screen::new(5, 2);
    assert_eq!(s.put(3, 0, "ab✓", HI, Some(BG), true), 6);
    assert_eq!(
        s.cell(3, 0),
        Cell {
            ch: 'a',
            fg: HI,
            bg: Some(BG),
            bold: true,
            ..Cell::default()
        }
    );
    assert_eq!(s.cell(4, 0).ch, 'b');
    assert_eq!(s.put(-1, 1, "xy", DIM, None, false), 1);
    assert_eq!(
        s.cell(0, 1),
        Cell {
            ch: 'y',
            fg: DIM,
            ..Cell::default()
        }
    );
    assert_eq!(s.text(), "   ab\ny");
}

#[test]
fn a_put_without_background_keeps_the_cells() {
    let mut s = Screen::new(3, 1);
    s.fill_bg(0, 0, 3, 1, BG);
    s.put(0, 0, "a", HI, None, false);
    assert_eq!(s.cell(0, 0).bg, Some(BG));
}

#[test]
fn the_last_target_wins_a_click_and_the_first_card_the_hover() {
    let mut s = Screen::new(10, 3);
    s.area(0, 10, 0, 2, Target::Orch("p1".into()));
    s.target(5, 7, 1, Target::AnswerOrch("p1".into()));
    assert_eq!(s.target_at(6, 1), Some(&Target::AnswerOrch("p1".into())));
    assert_eq!(s.target_at(4, 1), Some(&Target::Orch("p1".into())));
    assert_eq!(s.card_at(6, 1), Some(&Target::Orch("p1".into())));
    assert_eq!(s.target_at(6, 2), None);
}

#[test]
fn blit_copies_drawn_cells_and_shifts_targets() {
    let mut src = Screen::new(4, 10);
    src.put(1, 5, "ab", HI, None, false);
    src.target(0, 4, 5, Target::NewAgent);
    src.target(0, 4, 9, Target::NewAgent);
    let mut dst = Screen::new(4, 3);
    dst.put(0, 1, "zzzz", DIM, None, false);
    dst.blit(&src, 4, 0, 0, 3);
    assert_eq!(dst.text(), "\nzabz\n");
    assert_eq!(
        dst.targets,
        vec![Hit {
            x0: 0,
            x1: 4,
            y: 1,
            target: Target::NewAgent
        }]
    );
}

#[test]
fn rows_are_made_on_first_write() {
    let mut s = Screen::new(3, 400);
    assert!(!s.drawn(399));
    s.put(0, 399, "a", HI, None, false);
    assert!(s.drawn(399) && !s.drawn(398));
}
