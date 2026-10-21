//! The app and what is drawn over it (Python `SwarmApp` with its `Grid`): keys and the pointer
//! go to the app, the notices go over its frame, and each paint feeds the click arming.

use super::model::App;
use crate::ui::frame::Frame;
use crate::ui::input::{Clicks, Effect, Key, Painted};
use crate::ui::toast::{Placed, Toasts};
use panorama_core::view::Screen;
use std::time::Instant;

/// Rows a wheel step scrolls (Python `WHEEL_ROWS`).
pub const WHEEL_ROWS: i32 = 3;

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Click,
    Move,
    /// The pointer left the view.
    Leave,
    /// The wheel, down (`rows` > 0) or up; with shift, sideways.
    Wheel {
        down: bool,
        shift: bool,
    },
    /// A horizontal wheel, right or left.
    Sideways {
        right: bool,
    },
}

/// The app, its notices, and the view as last drawn (its click targets, without the notices).
pub struct Driver {
    pub app: App,
    pub toasts: Toasts,
    toast_rects: Vec<Placed>,
    shown: Option<Screen>,
}

impl Driver {
    pub fn new(app: App) -> Self {
        Self {
            app,
            toasts: Toasts::default(),
            toast_rects: vec![],
            shown: None,
        }
    }

    /// The view as last drawn, without the notices (its click targets).
    pub fn shown(&self) -> Option<&Screen> {
        self.shown.as_ref()
    }

    /// A key.
    pub fn key(&mut self, k: Key, now: Instant) -> Vec<Effect> {
        self.app.on_key(k, now)
    }

    /// The pointer at `(x, y)`: the effects, and how to repaint (`None`: nothing changed). A
    /// click on a notice dismisses it and goes no further; the wheel acts only over a column.
    pub fn pointer(
        &mut self,
        what: Pointer,
        x: i32,
        y: i32,
        now: Instant,
    ) -> (Vec<Effect>, Option<Painted>) {
        let Some(shown) = self.shown.take() else {
            return (vec![], None);
        };
        let over_column = shown
            .columns
            .iter()
            .any(|c| (c.x0..c.x1).contains(&x) && (c.y0..c.y1).contains(&y));
        let out = match what {
            Pointer::Click if self.toasts.dismiss_at(x, y, &self.toast_rects) => {
                (vec![], Some(Painted::Other))
            }
            Pointer::Click => (self.app.click(x, y, &shown, now), Some(Painted::Other)),
            Pointer::Move => (
                vec![],
                self.app
                    .hover(Some((x, y)), &shown)
                    .then_some(Painted::Other),
            ),
            Pointer::Leave => (
                vec![],
                self.app.hover(None, &shown).then_some(Painted::Other),
            ),
            Pointer::Wheel { down, shift: true } if over_column => (
                vec![],
                Some(self.app.hwheel(if down { 1 } else { -1 }, &shown)),
            ),
            Pointer::Wheel { down, shift: false } => {
                let rows = if down { WHEEL_ROWS } else { -WHEEL_ROWS };
                (
                    vec![],
                    self.app.wheel(x, y, rows, &shown).then_some(Painted::Wheel),
                )
            }
            Pointer::Sideways { right } if over_column => (
                vec![],
                Some(self.app.hwheel(if right { 1 } else { -1 }, &shown)),
            ),
            Pointer::Wheel { .. } | Pointer::Sideways { .. } => (vec![], None),
        };
        self.shown = Some(shown);
        out
    }

    /// The frame at `w` × `h` with the notices over it (`None` while the pane has no size);
    /// the click arming learns what changed (`cause`).
    pub fn paint(
        &mut self,
        w: i32,
        h: i32,
        look: &Frame,
        cause: Painted,
        now: Instant,
    ) -> Option<Screen> {
        let view = self.app.paint(w, h, look)?;
        self.app.stamp(self.shown.as_ref(), &view, cause, now);
        let mut screen = view.clone();
        self.toast_rects = self.toasts.draw(&mut screen);
        self.shown = Some(view);
        Some(screen)
    }
}

#[cfg(test)]
mod tests;
