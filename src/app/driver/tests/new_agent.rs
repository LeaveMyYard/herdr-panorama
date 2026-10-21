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
    let mut tick = look(r.now_s);
    tick.frame = 1.0; // a colour tick under the modal
    let (w, h) = r.size;
    assert!(r.d.paint(w, h, &tick, Painted::Other, now).is_some());
    assert_eq!(r.d.app.view.frame, 1.0);
    assert!(r.d.form.is_some());
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

#[test]
fn a_started_agent_is_said_and_a_popup_closes_over_its_tab() {
    // Python `_new_agent_done`: "started <name>" (title "new agent", 6 s); a popup exits
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE); // the home view
    let now = r.settled();
    r.d.open_form("/work", now - ARM);
    assert!(
        !r.d.agent_started(Err("boom".into()), now),
        "an error keeps the modal"
    );
    assert!(r.d.form.is_some());
    assert!(
        !r.d.agent_started(Ok("docs".into()), now),
        "the home view stays"
    );
    assert!(r.d.form.is_none());
    assert_eq!(r.d.toasts.next_expiry(), Some(now + Duration::from_secs(6)));
    let mut s = panorama_core::view::Screen::new(160, 45);
    r.d.toasts.draw(&mut s);
    assert!(
        s.text().contains("started docs") && s.text().contains("new agent"),
        "{}",
        s.text()
    );
    let (mut p, _) = opened("/work"); // Run::demo: a popup
    assert!(p.d.agent_started(Ok("docs".into()), now), "a popup closes");
    assert!(
        !p.d.agent_started(Ok("docs".into()), now),
        "nothing open: nothing to say"
    );
}

#[test]
fn a_click_outside_the_modal_reaches_nothing_behind_it() {
    // tests/test_mouse.py::test_new_agent_row_opens_the_form_only_when_armed, the form's half:
    // with the form open, a click on a view target outside it does nothing
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    let at = r.spot(&orch("w1:p2")).expect("the orchestrator is drawn");
    r.d.open_form("/work", now - ARM);
    let sel = format!("{:?}", r.sel());
    assert_eq!(
        r.d.form_click(at.0, at.1),
        FormStep::Nothing,
        "no modal picture yet"
    );
    let mut frame = r.d.shown().unwrap().clone();
    r.d.form_over(&mut frame);
    assert_eq!(r.d.form_click(at.0, at.1), FormStep::Nothing);
    assert_eq!(
        format!("{:?}", r.sel()),
        sel,
        "the view's cursor never moved"
    );
    assert!(r.d.form.is_some(), "the modal stays open");
}

#[test]
fn the_form_opens_on_the_default_directory_and_esc_returns_to_the_view() {
    // tests/test_tui.py::test_new_row_is_last_stable_and_opens_the_form, the form's half: the
    // + row's NewAgent { cwd: None } reaches the form as the home directory (the link's
    // `new_agent`); Esc closes it and the view goes on
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    r.d.open_form("/home/op", now - ARM);
    let mut frame = r.d.shown().unwrap().clone();
    r.d.form_over(&mut frame);
    assert_eq!(r.d.form.as_ref().unwrap().form.cwd.value, "/home/op");
    assert!(frame.text().contains("/home/op"));
    assert_eq!(r.d.form_key(FormKey::Esc, now), FormStep::Closed);
    assert!(r.d.form.is_none());
    let sel = format!("{:?}", r.sel());
    r.key(Key::Move(1), now); // the view's keys again
    assert_ne!(format!("{:?}", r.sel()), sel);
    let shown = r.d.shown().unwrap().clone();
    let mut after = shown.clone();
    r.d.form_over(&mut after);
    assert_eq!(after.text(), shown.text(), "no modal over the view");
    assert!(!after.targets.is_empty(), "the view takes clicks again");
}

mod routing {
    //! While the modal is open every key and click is the modal's: typing a name that holds
    //! the view's keys (`q`, `n`, `f`, `i`) never quits, filters or opens anything.
    use super::*;
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    fn key(c: KeyCode) -> Event {
        Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
    }

    fn mouse(kind: MouseEventKind, x: u16, y: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn typing_quinn_names_the_agent_and_never_quits_the_view() {
        let (mut r, now) = opened("/work");
        let (sel, filter) = (format!("{:?}", r.sel()), r.d.app.view.filter);
        for c in "quinnfi".chars() {
            assert_eq!(
                r.d.modal_input(&key(KeyCode::Char(c)), now),
                Some(FormStep::Repaint),
                "{c}"
            );
        }
        assert_eq!(r.d.form.as_ref().unwrap().form.name.value, "quinnfi");
        assert_eq!(
            (format!("{:?}", r.sel()), r.d.app.view.filter),
            (sel, filter)
        );
        // a key the form does not read is still the modal's
        assert_eq!(
            r.d.modal_input(&key(KeyCode::F(5)), now),
            Some(FormStep::Nothing)
        );
        assert_eq!(
            r.d.modal_input(&key(KeyCode::Esc), now),
            Some(FormStep::Closed)
        );
        // closed: the view's again
        assert_eq!(r.d.modal_input(&key(KeyCode::Char('q')), now), None);
    }

    #[test]
    fn the_pointer_never_reaches_the_view_behind() {
        let (mut r, now) = opened("/work");
        let at = r.spot(&orch("w1:p2")).expect("drawn");
        let mut frame = r.d.shown().unwrap().clone();
        r.d.form_over(&mut frame);
        let sel = format!("{:?}", r.sel());
        let (x, y) = (at.0 as u16, at.1 as u16);
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollRight,
            MouseEventKind::Moved,
        ] {
            assert_eq!(
                r.d.modal_input(&mouse(kind, x, y), now),
                Some(FormStep::Nothing),
                "{kind:?}"
            );
        }
        assert_eq!(format!("{:?}", r.sel()), sel);
        let (w, h) = WIDE;
        let open = &r.d.form.as_ref().unwrap().form;
        let modal = panorama_core::view::new_agent::render(&open.form(&open.view()), w, h);
        let next = modal.steps.iter().find(|s| s.d == 1).unwrap();
        let right = mouse(
            MouseEventKind::Down(MouseButton::Right),
            next.x as u16,
            next.y as u16,
        );
        assert_eq!(
            r.d.modal_input(&right, now),
            Some(FormStep::Nothing),
            "a left click only, as the view's"
        );
        let click = mouse(
            MouseEventKind::Down(MouseButton::Left),
            next.x as u16,
            next.y as u16,
        );
        assert_eq!(
            r.d.modal_input(&click, now),
            Some(FormStep::Repaint),
            "its own carousel"
        );
        // a resize or focus change is the view's even with the modal open
        assert_eq!(r.d.modal_input(&Event::Resize(80, 24), now), None);
        assert_eq!(r.d.modal_input(&Event::FocusLost, now), None);
    }

    #[test]
    fn with_no_modal_every_event_is_the_views() {
        let mut r = Run::demo("w1:p1", WIDE);
        let now = r.settled();
        assert_eq!(r.d.modal_input(&key(KeyCode::Char('q')), now), None);
        assert_eq!(
            r.d.modal_input(&mouse(MouseEventKind::Down(MouseButton::Left), 1, 1), now),
            None
        );
    }
}
