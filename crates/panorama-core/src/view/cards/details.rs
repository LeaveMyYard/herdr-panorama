//! An agent's detail lines (Python `render.agent_detail_lines`, `bg_row`): what it does now,
//! its background tasks, its questions and prompts, its error, and its last message.

use crate::model::{Agent, BgTask, Status};
use crate::view::anim::clock;
use crate::view::palette::{DIM, FAINT, FG, status};
use crate::view::seg::Seg;
use crate::view::text::{dur, fit, trunc, wrap};

/// A background task in `w` cells: its clock, kind, description and age on the right.
pub fn bg_row(b: &BgTask, w: i32, now: f64, frame: f64, i: usize) -> Vec<Seg> {
    let when = b
        .started
        .filter(|s| *s != 0.0)
        .map_or(String::new(), |s| dur(Some(now - s)));
    let room = (w - 11 - when.chars().count() as i32).max(1);
    let desc = if b.description.is_empty() {
        "in the background"
    } else {
        b.description.as_str()
    };
    vec![
        Seg::new(
            clock(frame, i as i64 + 1).to_string(),
            status(&Status::Background),
        )
        .bold(),
        Seg::plain(" "),
        Seg::new(fit(&b.kind, 8), DIM),
        Seg::new(fit(desc, room), FG),
        Seg::plain(" "),
        Seg::new(when, DIM),
    ]
}

/// A label for a status as Python's `LABEL.get(status, "")` reads it.
pub fn label(st: &Status) -> &str {
    if matches!(st, Status::Unknown(_)) {
        ""
    } else {
        st.label()
    }
}

/// Up to four wrapped lines, the first led by `lead`, the rest by two spaces.
fn led(text: &str, w: i32, lead: &str, st: &Status) -> Vec<Vec<Seg>> {
    let col = status(st);
    wrap(text, w - 2)
        .into_iter()
        .take(4)
        .enumerate()
        .map(|(i, line)| {
            vec![
                Seg::new(if i > 0 { "  " } else { lead }, col).bold(),
                Seg::new(line, col),
            ]
        })
        .collect()
}

/// The lines, `w` cells wide.
pub fn agent_detail_lines(a: &Agent, w: i32, now: f64, frame: f64) -> Vec<Vec<Seg>> {
    let doing = if a.activity.is_empty() {
        label(&a.status)
    } else {
        a.activity.as_str()
    };
    let mut out = vec![vec![
        Seg::new("now  ", FAINT),
        Seg::new(trunc(doing, w - 5), FG),
    ]];
    if a.status == Status::Background {
        out.extend(
            a.bg.iter()
                .enumerate()
                .map(|(i, b)| bg_row(b, w - 1, now, frame, i)),
        );
    }
    for q in &a.questions {
        out.extend(led(q, w, "? ", &Status::Waiting));
    }
    for p in &a.pending {
        match p.strip_prefix("wants ") {
            Some(what) => out.push(vec![
                Seg::new("! ", status(&Status::Blocked)).bold(),
                Seg::new("wants ", DIM),
                Seg::new(trunc(what, w - 8), status(&Status::Blocked)).bold(),
            ]),
            None => out.extend(led(p, w, "! ", &Status::Blocked)),
        }
    }
    if let Some(err) = a.error.as_deref().filter(|e| !e.is_empty()) {
        let col = status(&Status::Failed);
        out.push(vec![
            Seg::new("✗ ", col).bold(),
            Seg::new(trunc(err, w - 2), col),
        ]);
    }
    if !a.last_message.is_empty()
        && a.questions.is_empty()
        && a.error.as_deref().is_none_or(str::is_empty)
    {
        out.extend(
            wrap(&format!("“{}”", a.last_message), w)
                .into_iter()
                .take(2)
                .map(|l| vec![Seg::new(l, DIM)]),
        );
    }
    out
}
