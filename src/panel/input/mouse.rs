//! The mouse (Python `Grid.on_click`, `on_mouse_move`, the wheel, `SwarmApp.click_target`,
//! `hover`, `wheel`): a click acts on the identity drawn under it at the last paint, only if
//! that identity is still shown, never on a cell that changed in the last refresh or scroll.
//! Selection clicks only move the cursor; action clicks are armed like their keys.

use super::effect::Effect;
use crate::panel::model::Panel;
use crate::ui::input::Clicks;
use panorama_core::view::Screen;
use std::time::Instant;

/// Rows a wheel step scrolls.
pub const WHEEL_ROWS: i32 = 3;

impl Panel {
    /// A click at `(x, y)` on `screen` (the last one painted): [`Clicks::click`].
    pub fn click(&mut self, x: i32, y: i32, screen: &Screen, now: Instant) -> Vec<Effect> {
        Clicks::click(self, x, y, screen, now)
    }
}

#[cfg(test)]
mod tests;
