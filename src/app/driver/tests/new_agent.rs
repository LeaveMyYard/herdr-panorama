//! The + New agent modal in the app (Python tests/test_tui.py's form cases): over the dimmed
//! view, its keys never reach the view, refreshes go on under it, its start and its end.

use super::*;
use crate::app::form::FormStep;
use crate::new_agent::form::{ARM, FormKey};
use crate::new_agent::overlay::SHADE;

const WIDE: (i32, i32) = (160, 45);

fn opened(dir: &str) -> (Run, Instant) {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.d.open_form(dir, now - ARM);
    r.paint(Painted::Other, now);
    (r, now)
}

fn typed(r: &mut Run, text: &str, now: Instant) {
    for c in text.chars() {
        r.d.form_key(FormKey::Char(c), now);
    }
}

#[test]
fn the_modal_is_drawn_over_the_dimmed_view_and_the_view_takes_no_clicks() {
    let (mut r, _) = opened("/work");
    let s = r.d.shown().unwrap().clone(); // the view as drawn, without the modal: its targets kept
    assert!(!s.targets.is_empty());
    let mut frame = s.clone();
    r.d.form_over(&mut frame);
    assert!(frame.text().contains("+ New agent"), "{}", frame.text());
    assert!(frame.targets.is_empty(), "the view behind takes no clicks");
    // a cell outside the modal is the view's, dimmed toward the shade
    let (x, y) = (1, 1);
    let was = s.cell(x, y);
    let now = frame.cell(x, y);
    assert_eq!(now.ch, was.ch);
    assert_eq!(now.fg, was.fg.mix(SHADE, 0.55));
}

#[test]
fn its_keys_never_reach_the_view_and_refreshes_go_on_under_it() {
    // tests/test_tui.py r5 L1: arrows in the form do not move the view's cursor
    let (mut r, now) = opened("/work");
    let sel = format!("{:?}", r.sel());
    for k in [
        FormKey::Down,
        FormKey::Up,
        FormKey::Left,
        FormKey::Right,
        FormKey::Char('j'),
    ] {
        r.d.form_key(k, now);
    }
    assert_eq!(format!("{:?}", r.sel()), sel);
    let (swarm, _) = wire("demo");
    r.refresh(swarm, now); // a refresh under the modal
    assert!(r.d.form.is_some(), "the modal stays");
    let mut frame = r.d.shown().unwrap().clone();
    r.d.form_over(&mut frame);
    assert!(frame.text().contains("+ New agent"));
}

#[test]
fn a_start_ends_the_modal_and_a_failed_one_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut r, now) = opened(dir.path().to_str().unwrap());
    typed(&mut r, "docs", now);
    r.d.form_key(FormKey::Tab, now);
    let start = r.d.form_key(FormKey::Enter, now);
    assert_eq!(
        start,
        FormStep::Start {
            name: "docs".into(),
            cwd: Some(dir.path().to_str().unwrap().into()),
            kind: "claude"
        }
    );
    assert_eq!(
        r.d.form_started(Err("agent start failed: boom".into())),
        None
    );
    let f = &r.d.form.as_ref().unwrap().form;
    assert_eq!(f.status.as_deref(), Some("agent start failed: boom"));
    assert!(matches!(
        r.d.form_key(FormKey::Submit, now),
        FormStep::Start { .. }
    ));
    assert_eq!(r.d.form_started(Ok("docs".into())), Some("docs".into()));
    assert!(r.d.form.is_none(), "closed once it runs");
}

#[test]
fn esc_closes_it_and_a_second_open_keeps_the_first() {
    let (mut r, now) = opened("/work");
    typed(&mut r, "x", now);
    r.d.open_form("/elsewhere", now); // n again while it is open
    assert_eq!(r.d.form.as_ref().unwrap().form.name.value, "x");
    assert_eq!(r.d.form_key(FormKey::Esc, now), FormStep::Closed);
    assert!(r.d.form.is_none());
    assert_eq!(
        r.d.form_key(FormKey::Esc, now),
        FormStep::Nothing,
        "nothing open"
    );
}

#[test]
fn a_click_on_the_carousel_switches_the_harness() {
    let (mut r, _) = opened("/work");
    let mut frame = r.d.shown().unwrap().clone();
    r.d.form_over(&mut frame);
    let (w, h) = WIDE;
    let modal = panorama_core::view::new_agent::render(
        &r.d.form
            .as_ref()
            .unwrap()
            .form
            .form(&r.d.form.as_ref().unwrap().form.view()),
        w,
        h,
    );
    let next = modal.steps.iter().find(|s| s.d == 1).unwrap();
    assert_eq!(r.d.form_click(next.x, next.y), FormStep::Repaint);
    assert_eq!(r.d.form.as_ref().unwrap().form.harness, 1);
    assert_eq!(r.d.form_click(0, 0), FormStep::Nothing);
}
