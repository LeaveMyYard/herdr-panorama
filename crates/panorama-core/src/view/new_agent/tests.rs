use super::look::{BG, FIELD, TEXT, WARN};
use super::*;

fn form() -> Form<'static> {
    Form {
        harness: 0,
        name: "billing",
        slug: "billing",
        cwd: "~/x",
        focus: None,
        status: None,
    }
}

fn row(s: &Screen, y: i32) -> String {
    (0..s.w()).map(|x| s.cell(x, y).ch).collect()
}

#[test]
fn steps_are_the_ghosts_and_the_arrows_the_first_one_winning() {
    let m = render(&form(), W, H);
    assert_eq!(m.step_at(1, 2), Some(-1)); // the left ghost
    assert_eq!(m.step_at(W - 2, 10), Some(1)); // the right ghost
    assert_eq!(m.step_at(W / 2 - 20, 10), Some(-1)); // ‹
    assert_eq!(m.step_at(W / 2 + 20, 10), Some(1)); // ›
    assert_eq!(m.step_at(W / 2, 10), None); // the name
    assert_eq!(m.step_at(1, 11), None); // under the ghosts
}

#[test]
fn the_cursor_is_a_reversed_cell_where_it_stands() {
    let f = Form {
        focus: Some((Field::Name, 2)),
        ..form()
    };
    let m = render(&f, W, H);
    let c = m.screen.cell(3 + 2 + 2, 16);
    assert_eq!((c.ch, c.fg, c.bg), ('l', BG, Some(TEXT)));
    let after = m.screen.cell(3 + 2 + 3, 16);
    assert_eq!((after.ch, after.bg), ('l', Some(FIELD)));
    // the other field has no cursor
    assert!((0..W).all(|x| m.screen.cell(x, 22).bg != Some(TEXT)));
}

#[test]
fn a_long_value_scrolls_to_keep_the_cursor_inside_the_box() {
    let long = "x".repeat(100) + "END";
    let f = Form {
        cwd: &long,
        focus: Some((Field::Cwd, 103)),
        ..form()
    };
    let m = render(&f, W, H);
    let r = row(&m.screen, 22);
    // the cursor's blank in the box's last inside column, then the box and the frame
    assert!(r.ends_with("xEND │  │"), "{r:?}");
    assert_eq!(m.screen.cell(W - 5, 22).bg, Some(TEXT));
    let at_start = Form {
        cwd: &long,
        focus: Some((Field::Cwd, 0)),
        ..form()
    };
    let r = row(&render(&at_start, W, H).screen, 22);
    assert!(r.ends_with("xx│  │"), "{r:?}"); // cut at the box, never over it
    assert_eq!(r.chars().nth(5), Some('x'));
}

#[test]
fn no_agent_name_no_line_under_the_name_and_the_status_is_a_warning() {
    let f = Form {
        slug: "",
        status: Some("The name needs at least one letter."),
        ..form()
    };
    let m = render(&f, W, H);
    assert!(
        row(&m.screen, 18)
            .trim_matches(|c| c == '│' || c == ' ')
            .is_empty()
    );
    assert_eq!(
        row(&m.screen, H - 3).trim_matches(|c| c == '│' || c == ' '),
        "The name needs at least one letter."
    );
    assert_eq!(m.screen.cell(2, H - 3).fg, WARN);
    let long = "y".repeat(200);
    let m = render(
        &Form {
            status: Some(&long),
            ..form()
        },
        W,
        H,
    );
    assert_eq!(m.screen.cell(W - 1, H - 3).ch, '│'); // cut inside the frame
}

#[test]
fn the_modal_is_centred_and_a_small_screen_only_clips_it() {
    let m = render(&form(), W + 10, H + 4);
    assert_eq!(m.screen.cell(5, 2).ch, '╭');
    assert_eq!(m.step_at(6, 4), Some(-1));
    let small = render(&form(), 20, 5);
    assert_eq!(small.screen.cell(0, 0).ch, '╭');
}

#[test]
fn the_harness_steps_wrap_round() {
    assert_eq!(step(0, -1), HARNESSES.len() - 1);
    assert_eq!(step(HARNESSES.len() - 1, 1), 0);
    assert!(HARNESSES[0].supported() && HARNESSES[1..].iter().all(|h| !h.supported()));
}
