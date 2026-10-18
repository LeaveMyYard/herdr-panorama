//! Registering click targets on a screen, and finding what a click or the mouse hits
//! (Python `Screen.target`, `area`, `target_at`, `card_at`).

use super::Screen;
use crate::view::target::{Hit, Target};

impl Screen {
    /// A click target at `x0..x1` on row `y`.
    pub fn target(&mut self, x0: i32, x1: i32, y: i32, target: Target) {
        self.targets.push(Hit { x0, x1, y, target });
    }

    /// A whole card, rows `y0..y1`, as a click target. Register a card before the targets
    /// inside it (a later target wins).
    pub fn area(&mut self, x0: i32, x1: i32, y0: i32, y1: i32, target: Target) {
        for y in y0..y1 {
            self.target(x0, x1, y, target.clone());
        }
    }

    /// What a click at `(x, y)` hits: the last target registered there (Python `target_at`).
    pub fn target_at(&self, x: i32, y: i32) -> Option<&Target> {
        self.targets
            .iter()
            .rev()
            .find(|t| t.y == y && (t.x0..t.x1).contains(&x))
            .map(|t| &t.target)
    }

    /// The card at `(x, y)`, for the hover look: the first card registered there (Python
    /// `card_at`).
    pub fn card_at(&self, x: i32, y: i32) -> Option<&Target> {
        self.targets
            .iter()
            .find(|t| t.y == y && (t.x0..t.x1).contains(&x) && t.target.is_card())
            .map(|t| &t.target)
    }
}
