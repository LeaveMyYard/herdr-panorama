//! The terminal's events: keys, the pointer, resizes, focus in and out (Python `SwarmApp`'s
//! bindings and `Grid`'s mouse handlers), handed to the [`super::Driver`].

use super::{Actor, End};
use crate::app::driver::Pointer;
use crate::new_agent::keys::form_key;
use crate::ui::input::{Painted, key};
use crossterm::event::{Event as TermEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::io;
use std::time::Instant;

impl Actor {
    /// A terminal event; `Some` when the app ends.
    pub(super) fn on_terminal(
        &mut self,
        ev: Option<io::Result<TermEvent>>,
    ) -> io::Result<Option<End>> {
        let now = Instant::now();
        match ev {
            Some(Ok(TermEvent::Key(k))) if self.driver.form.is_some() => {
                let Some(k) = form_key(k) else {
                    return Ok(None);
                };
                let step = self.driver.form_key(k, now);
                return self.on_form(step);
            }
            Some(Ok(TermEvent::Mouse(m))) if self.driver.form.is_some() => {
                if m.kind != MouseEventKind::Down(MouseButton::Left) {
                    return Ok(None); // the view behind the modal takes no pointer
                }
                let step = self
                    .driver
                    .form_click(i32::from(m.column), i32::from(m.row));
                return self.on_form(step);
            }
            Some(Ok(TermEvent::Key(k))) => {
                let Some(k) = key(k) else { return Ok(None) };
                let effects = self.driver.key(k, now);
                if let Some(end) = self.act(effects) {
                    return Ok(Some(end));
                }
                self.publish_filter(); // a filter chosen here: every panel shows it
            }
            Some(Ok(TermEvent::Mouse(m))) => return self.on_mouse(m, now),
            Some(Ok(TermEvent::Resize(..))) => {}
            Some(Ok(TermEvent::FocusGained)) => self.driver.app.pane_focused = true,
            Some(Ok(TermEvent::FocusLost)) => self.driver.app.pane_focused = false,
            Some(Ok(_)) => return Ok(None),
            Some(Err(e)) => return Err(e),
            None => return Ok(Some(End::Input)),
        }
        self.repaint(Painted::Other).map(|()| None)
    }

    fn on_mouse(&mut self, m: MouseEvent, now: Instant) -> io::Result<Option<End>> {
        let shift = m.modifiers.contains(KeyModifiers::SHIFT);
        let what = match m.kind {
            MouseEventKind::Down(MouseButton::Left) => Pointer::Click,
            MouseEventKind::Moved | MouseEventKind::Drag(_) => Pointer::Move,
            MouseEventKind::ScrollUp => Pointer::Wheel { down: false, shift },
            MouseEventKind::ScrollDown => Pointer::Wheel { down: true, shift },
            MouseEventKind::ScrollLeft => Pointer::Sideways { right: false },
            MouseEventKind::ScrollRight => Pointer::Sideways { right: true },
            _ => return Ok(None),
        };
        let (effects, repaint) =
            self.driver
                .pointer(what, i32::from(m.column), i32::from(m.row), now);
        if let Some(end) = self.act(effects) {
            return Ok(Some(end));
        }
        self.publish_filter();
        match repaint {
            Some(cause) => self.repaint(cause).map(|()| None),
            None => Ok(None),
        }
    }
}
