//! The status legend: tiles on a wide screen, coloured glyphs and short words on a narrow
//! one, which drops whole entries when it does not fit.

use crate::model::Status;
use crate::view::palette::{FAINT, status};
use crate::view::pod::pod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;

/// The legend: (status, glyph, words), in order.
const LEGEND: [(Status, &str, &str); 11] = [
    (Status::Done, "✓", "done"),
    (Status::Inferred, "✓?", "done?"),
    (Status::Running, "⠋", "running"),
    (Status::Reviewing, "»", "review"),
    (Status::Background, "⠁", "waiting"),
    (Status::Queued, "○", "queued"),
    (Status::Stale, "◌", "stale"),
    (Status::Waiting, "?", "question"),
    (Status::Blocked, "!", "blocked"),
    (Status::Failed, "✗", "failed"),
    (Status::Lost, "~", "lost"),
];

/// A narrow legend's shorter words.
fn short(label: &str) -> &str {
    match label {
        "running" => "run",
        "question" => "ask",
        "blocked" => "block",
        "failed" => "fail",
        "queued" => "queue",
        "review" => "rev",
        "waiting" => "wait",
        other => other,
    }
}

/// A narrow legend that does not fit drops whole entries, these first.
const NARROW_DROP: [Status; 4] = [
    Status::Stale,
    Status::Background,
    Status::Lost,
    Status::Inferred,
];

/// The legend at row `y`: tiles for the wide layout (`wide`), coloured glyphs and short words
/// for the narrow one.
pub fn legend(s: &mut Screen, y: i32, wide: bool) {
    let mut items: Vec<&(Status, &str, &str)> = LEGEND.iter().collect();
    let gap = if s.w() >= 80 { 2 } else { 1 };
    if !wide {
        let len = |t: &str| t.chars().count() as i32;
        let width = |its: &[&(Status, &str, &str)]| {
            1 + its
                .iter()
                .map(|(_, g, lab)| len(g) + 1 + len(short(lab)) + gap)
                .sum::<i32>()
        };
        for k in &NARROW_DROP {
            if width(&items) - gap < s.w() {
                break;
            }
            items.retain(|it| it.0 != *k);
        }
    }
    let mut x = 1;
    for (st, g, lab) in items {
        if wide {
            pod(s, x, y, st, g, false);
            x = s.segs(x + 4, y, &[Seg::new(*lab, FAINT)]) + 2;
        } else {
            let segs = [
                Seg::new(*g, status(st)).bold(),
                Seg::new(format!(" {}", short(lab)), FAINT),
            ];
            x = s.segs(x, y, &segs) + gap;
        }
    }
}
