//! The + New agent modal's keys and clicks (Python `NewAgentForm`; design 1C).

use crate::new_agent::form::{ARM, FormKey, NewAgentForm, Outcome};
use panorama_core::view::new_agent::{Field, HARNESSES, render};
use std::time::Instant;

fn typed(f: &mut NewAgentForm, text: &str, now: Instant) {
    for c in text.chars() {
        f.key(FormKey::Char(c), now);
    }
}

#[test]
fn typing_edits_the_focused_field_and_shows_the_agent_name() {
    let t0 = Instant::now();
    let mut f = NewAgentForm::new("/work", t0);
    assert_eq!(f.focus, Field::Name);
    typed(&mut f, "Billing API", t0);
    assert_eq!(f.view().slug, "billing-api");
    f.key(FormKey::Left, t0);
    f.key(FormKey::Backspace, t0); // "Billing AI"
    f.key(FormKey::Home, t0);
    f.key(FormKey::Delete, t0);
    assert_eq!(f.name.value, "illing AI");
    f.key(FormKey::End, t0);
    typed(&mut f, "!", t0);
    assert_eq!(f.name.value, "illing AI!");
    assert_eq!(
        f.key(FormKey::Char('\u{7}'), t0),
        Outcome::Nothing,
        "control characters are not typed"
    );
    // Enter on the name, tab and shift-tab move between the two fields; the directory is filled
    assert_eq!(f.key(FormKey::Enter, t0), Outcome::Changed);
    assert_eq!(f.focus, Field::Cwd);
    assert_eq!((f.cwd.value.as_str(), f.cwd.cursor), ("/work", 5));
    f.key(FormKey::BackTab, t0);
    assert_eq!(f.focus, Field::Name);
    f.key(FormKey::Tab, t0);
    assert_eq!(f.focus, Field::Cwd);
}

#[test]
fn enter_on_the_last_field_or_ctrl_s_starts_once_armed() {
    let t0 = Instant::now();
    let dir = tempfile::tempdir().unwrap();
    let mut f = NewAgentForm::new(dir.path().to_str().unwrap(), t0);
    typed(&mut f, "docs", t0);
    assert_eq!(
        f.key(FormKey::Submit, t0),
        Outcome::Nothing,
        "not armed yet"
    );
    let later = t0 + ARM;
    f.key(FormKey::Tab, later);
    let started = f.key(FormKey::Enter, later);
    assert_eq!(
        started,
        Outcome::Start {
            name: "docs".into(),
            cwd: Some(dir.path().to_str().unwrap().into()),
            kind: "claude"
        }
    );
    assert!(f.busy);
    assert_eq!(f.status.as_deref(), Some("starting Claude Code…"));
    let view = f.view();
    assert_eq!(f.form(&view).focus, None, "nothing focused while it starts");
    assert_eq!(
        f.key(FormKey::Esc, later),
        Outcome::Nothing,
        "a start is never cancelled half-way"
    );
    assert_eq!(
        f.key(FormKey::Submit, later),
        Outcome::Nothing,
        "and never started twice"
    );
    assert_eq!(f.finished(Err("agent start failed: boom".into())), None);
    assert!(!f.busy && f.status.as_deref() == Some("agent start failed: boom"));
    assert!(
        matches!(f.key(FormKey::Submit, later), Outcome::Start { .. }),
        "it can be tried again"
    );
    assert_eq!(f.finished(Ok("docs".into())), Some("docs".into()));
}

#[test]
fn a_bad_name_or_directory_is_said_on_the_status_row() {
    let later = Instant::now() + ARM;
    let mut f = NewAgentForm::new("", Instant::now() - ARM);
    typed(&mut f, "!!!", later);
    assert_eq!(f.key(FormKey::Submit, later), Outcome::Changed);
    assert_eq!(
        f.status.as_deref(),
        Some("The name needs at least one letter.")
    );
    let dir = tempfile::tempdir().unwrap();
    let nope = dir.path().join("nope");
    let mut f = NewAgentForm::new(&format!("  {}  ", nope.display()), Instant::now() - ARM);
    typed(&mut f, "docs", later);
    assert_eq!(f.key(FormKey::Submit, later), Outcome::Changed);
    assert_eq!(
        f.status,
        Some(format!("No such directory: {}", nope.display()))
    );
    assert!(!f.busy);
    let mut f = NewAgentForm::new("   ", Instant::now() - ARM);
    typed(&mut f, "docs", later);
    assert_eq!(
        f.key(FormKey::Submit, later),
        Outcome::Start {
            name: "docs".into(),
            cwd: None,
            kind: "claude"
        }
    );
}

#[test]
fn esc_cancels_and_a_harness_that_is_coming_soon_starts_nothing() {
    let later = Instant::now() + ARM;
    let mut f = NewAgentForm::new("", Instant::now() - ARM);
    typed(&mut f, "docs", later);
    let modal = render(&f.form(&f.view()), 96, 32);
    let next = modal.steps.iter().find(|s| s.d == 1).unwrap();
    assert_eq!(f.click(next.x, next.y, &modal), Outcome::Changed);
    assert_ne!(f.harness, 0);
    assert!(!HARNESSES[f.harness].supported());
    assert_eq!(f.key(FormKey::Submit, later), Outcome::Nothing);
    assert!(f.status.is_none() && !f.busy);
    let modal = render(&f.form(&f.view()), 96, 32);
    let back = modal.steps.iter().find(|s| s.d == -1).unwrap();
    f.click(back.x, back.y, &modal);
    assert_eq!(f.harness, 0);
    assert_eq!(
        f.click(0, 0, &modal),
        Outcome::Nothing,
        "outside the carousel"
    );
    assert_eq!(f.key(FormKey::Esc, later), Outcome::Cancel);
}

#[test]
fn the_carousel_holds_still_while_the_agent_starts() {
    let later = Instant::now() + ARM;
    let mut f = NewAgentForm::new("", Instant::now() - ARM);
    typed(&mut f, "docs", later);
    let modal = render(&f.form(&f.view()), 96, 32);
    assert!(matches!(
        f.key(FormKey::Submit, later),
        Outcome::Start { .. }
    ));
    let next = modal.steps.iter().find(|s| s.d == 1).unwrap();
    assert_eq!(f.click(next.x, next.y, &modal), Outcome::Nothing);
    assert_eq!(f.harness, 0, "the harness being started stays");
}
