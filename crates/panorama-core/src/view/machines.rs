//! Orchestrators on another herdr machine, grouped under a divider with the machine's label
//! (Python `render._group_start`, `machine_divider`; the operator's choice).

use super::palette::{FAINT, LINE};
use super::screen::Screen;
use super::text::trunc;
use crate::model::Orchestrator;

/// The machine label when `o` starts another machine's group (local ones have none). Groups
/// are keyed by the machine's id: two machines may share a label.
pub fn group_start<'a>(o: &'a Orchestrator, prev: &mut Option<&'a str>) -> Option<&'a str> {
    // Python's `or`: an empty id or label counts as none
    let set = |s: &'a Option<String>| s.as_deref().filter(|s| !s.is_empty());
    let key = set(&o.machine_id).or(set(&o.machine));
    let starts = key.is_some() && key != *prev;
    *prev = key;
    if starts {
        set(&o.machine).or(key)
    } else {
        None
    }
}

/// A faint divider row with a machine's label above its group.
pub fn divider(cv: &mut Screen, y: i32, w: i32, label: &str) {
    let text = trunc(&format!(" {label} "), (w - 2).max(0));
    let n = text.chars().count() as i32;
    cv.put(0, y, "─", LINE, None, false);
    cv.put(1, y, &text, FAINT, None, false);
    cv.put(
        1 + n,
        y,
        &"─".repeat((w - 1 - n).max(0) as usize),
        LINE,
        None,
        false,
    );
}

#[cfg(test)]
mod tests;
