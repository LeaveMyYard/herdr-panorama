//! The actor's decisions, without its terminal, sockets or tasks (so they are tested): what
//! a state, a key, the pointer or a report from the herdr side does to the panel and its
//! notices, and what a paint draws and arms.

use super::effects::Done;
use crate::panel::act::goto;
use crate::panel::input::keys::Key;
use crate::panel::input::mouse::WHEEL_ROWS;
use crate::panel::input::{Effect, Painted, Toast};
use crate::panel::model::Panel;
use crate::panel::paint::Frame;
use crate::proto::state::State;
use crate::ui::toast::{Placed, Toasts};
use panorama_core::view::Screen;
use std::time::Instant;

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Click,
    Move,
    WheelUp,
    WheelDown,
}

/// The panel and what is drawn over it.
pub struct Driver {
    pub panel: Panel,
    pub toasts: Toasts,
    toast_rects: Vec<Placed>,
    /// The view as last drawn (its click targets), without the notices.
    shown: Option<Screen>,
    /// The operator's navigation so far: a pane check that comes back after a newer move,
    /// goto or switch is dropped (Python checked synchronously, before any later key).
    nav: u64,
}

/// What the operator's selection is, to see it move.
fn selected(p: &Panel) -> (Option<String>, Option<String>, Option<String>, usize) {
    let v = &p.view;
    (v.orch.clone(), v.thread.clone(), v.agent.clone(), v.focus)
}

impl Driver {
    pub fn new(panel: Panel) -> Self {
        Self {
            panel,
            toasts: Toasts::default(),
            toast_rects: vec![],
            shown: None,
            nav: 0,
        }
    }

    /// A state from the core: a refresh (cells whose click target moved wait).
    pub fn state(&mut self, st: State, now: Instant) -> Painted {
        self.panel.on_state(st, now);
        Painted::Refresh
    }

    /// A key.
    pub fn key(&mut self, k: Key, now: Instant) -> Vec<Effect> {
        self.navigating(|p| p.on_key(k, now))
    }

    /// The pointer at `(x, y)`: the effects, and how to repaint (`None`: nothing changed). A
    /// click on a notice dismisses it and goes no further.
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
        let out = match what {
            Pointer::Click if self.toasts.dismiss_at(x, y, &self.toast_rects) => {
                (vec![], Some(Painted::Other))
            }
            Pointer::Click => (
                self.navigating(|p| p.click(x, y, &shown, now)),
                Some(Painted::Other),
            ),
            Pointer::Move => (
                vec![],
                self.panel
                    .hover(Some((x, y)), &shown)
                    .then_some(Painted::Other),
            ),
            Pointer::WheelUp => (
                vec![],
                self.panel
                    .wheel(x, y, -WHEEL_ROWS, &shown)
                    .then_some(Painted::Wheel),
            ),
            Pointer::WheelDown => (
                vec![],
                self.panel
                    .wheel(x, y, WHEEL_ROWS, &shown)
                    .then_some(Painted::Wheel),
            ),
        };
        self.shown = Some(shown);
        out
    }

    fn navigating(&mut self, f: impl FnOnce(&mut Panel) -> Vec<Effect>) -> Vec<Effect> {
        let before = selected(&self.panel);
        let out = f(&mut self.panel);
        if selected(&self.panel) != before {
            self.nav += 1;
        }
        out
    }

    /// A goto or switch goes out: newer than every pane check before it; its number.
    pub fn launched(&mut self) -> u64 {
        self.nav += 1;
        self.nav
    }

    /// A report from the herdr side.
    pub fn done(&mut self, done: Done, now: Instant) -> Vec<Effect> {
        match done {
            Done::Switched(s) => self.panel.switched(s.err, s.took_back, now),
            // the operator moved on while herdr answered: not where they are going now
            Done::Checked { nav, .. } if nav != self.nav => vec![],
            Done::Checked {
                label,
                exists: false,
                ..
            } => vec![Effect::Toast(Toast::new(None, goto::gone(&label), 3))],
            Done::Checked {
                pane,
                keep_focus,
                seen,
                ..
            } => {
                let mut out = self.panel.seen_now(seen);
                out.extend(self.panel.switch(pane, keep_focus));
                out
            }
            Done::Toast(t) => vec![Effect::Toast(t)],
        }
    }

    /// Paints the view at `w` × `h` with the notices over it; the click arming learns what
    /// changed (`cause`).
    pub fn paint(&mut self, w: i32, h: i32, look: &Frame, cause: Painted, now: Instant) -> Screen {
        let view = self.panel.paint(w, h, look);
        self.panel.painted(self.shown.as_ref(), &view, cause, now);
        let mut screen = view.clone();
        self.toast_rects = self.toasts.draw(&mut screen);
        self.shown = Some(view);
        screen
    }
}

#[cfg(test)]
mod tests;
