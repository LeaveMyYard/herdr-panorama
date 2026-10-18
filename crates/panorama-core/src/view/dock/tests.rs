use super::*;
use crate::model::Status;
use crate::view::palette::{FAINT, FG};
use crate::view::theme::PanelLook;

#[test]
fn the_theme_leaves_animated_colours_as_drawn() {
    // a needs-you glyph in its off phase is FAINT, as a plain faint text is: only the plain
    // one is the theme's (Python skips Anim colours)
    let mut s = Screen::new(3, 1);
    s.fill_bg(0, 0, 3, 1, BG);
    s.put(0, 0, "a", FAINT, None, false);
    s.put_seg(1, 0, &Seg::new("!", FAINT).fg_moving(true));
    s.fill_bg_moving(2, 0, 1, 1, crate::view::palette::SEL, true);
    let to = crate::view::Rgb::hex(0x565f89);
    let look = PanelLook {
        bg: crate::view::Rgb::hex(0x1a1b26),
        fgs: vec![(FAINT, to), (FG, to)],
        hover: BG,
    };
    look::apply(&mut s, &look);
    assert_eq!((s.cell(0, 0).fg, s.cell(0, 0).bg), (to, Some(look.bg)));
    assert_eq!((s.cell(1, 0).fg, s.cell(1, 0).bg), (FAINT, Some(look.bg)));
    assert_eq!(s.cell(2, 0).bg, Some(crate::view::palette::SEL)); // a moving background: untouched
}

#[test]
fn the_list_follows_the_selection_unless_the_wheel_moved_it() {
    let mut v = View::default();
    assert_eq!(scroll(&v, Some((50, 54)), 41, 80), 13);
    assert_eq!(scroll(&v, Some((0, 2)), 41, 80), 0);
    v.offsets.insert(COLUMN, 30);
    assert_eq!(scroll(&v, Some((0, 2)), 41, 80), 0);
    v.wheel.insert(COLUMN);
    assert_eq!(scroll(&v, Some((0, 2)), 41, 80), 30);
    assert_eq!(scroll(&v, None, 41, 50), 9); // never past the content
}

#[test]
fn machine_groups_start_once_per_machine_id() {
    let o = |id: Option<&str>, label: Option<&str>| -> Orchestrator {
        serde_json::from_value(
            serde_json::json!({"pane_id": "p", "tab_id": null, "tab_label": "x",
            "session_id": null, "machine_id": id, "machine": label}),
        )
        .unwrap()
    };
    let (local, a1, a2, b) = (
        o(None, None),
        o(Some("m1"), Some("box")),
        o(Some("m1"), Some("box")),
        o(Some("m2"), Some("box")),
    );
    let mut prev = None;
    let starts: Vec<Option<&str>> = [&local, &a1, &a2, &b]
        .into_iter()
        .map(|o| group_start(o, &mut prev))
        .collect();
    // two machines may share a label: each still gets its divider
    assert_eq!(starts, [None, Some("box"), None, Some("box")]);
    let mut cv = Screen::new(12, 1);
    machine_divider(&mut cv, 0, 12, "box");
    assert_eq!(cv.text(), "─ box ──────");
}

#[test]
fn the_footer_fills_in_the_open_key_and_drops_whole_hints_when_narrow() {
    let line = |w: i32, key: (&str, &str)| {
        let v = View {
            open_key: (key.0.into(), key.1.into()),
            ..View::default()
        };
        let mut s = Screen::new(w, 3);
        chrome::footer(&mut s, &v, true);
        s.text().lines().last().unwrap().to_owned()
    };
    assert_eq!(
        line(46, ("pfx+spc", "pfx+spc")),
        " ↑↓ move ret open a answer n new pfx+spc back"
    );
    // a long key: whole without `n new`, then its short form
    assert_eq!(
        line(46, ("ctrl+b space", "^b spc")),
        " ↑↓ move ret open a answer ctrl+b space back"
    );
    assert_eq!(
        line(40, ("ctrl+b space", "^b spc")),
        " ↑↓ move ret open a answer ^b spc back"
    );
    assert_eq!(
        line(30, ("pfx+spc", "pfx+spc")),
        " ↑↓ move ret open pfx+spc back"
    );
}

#[test]
fn many_thread_tiles_end_in_plus_n_and_the_counts_stay_whole() {
    let t = |g: &str| tiles::tile(&Status::Running, g, 0.0, None);
    let line = tiles::fit_tiles(vec![t("a"), t("b"), t("c"), t("d")], "  4 active", 20);
    let text: String = line.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, " a   b  +2  4 active");
    let line = tiles::fit_tiles(vec![t("a")], "  1 active · 9 agents", 8);
    assert_eq!(
        line.iter().map(|s| s.text.as_str()).collect::<String>(),
        "1 activ…"
    );
}
