//! The terminal's events: keys, the mouse, resizes, focus in and out (Python `SwarmApp`'s
//! bindings and `Grid`'s mouse handlers), handed to the [`super::Driver`].

use super::driver::Pointer;
use super::{Actor, End};
use crate::panel::input::Painted;
use crate::panel::input::keys::key;
use crossterm::event::{Event as TermEvent, MouseButton, MouseEvent, MouseEventKind};
use std::io;
use std::time::Instant;

impl Actor {
    /// A terminal event; `Some` when the panel ends.
    pub(super) async fn on_terminal(
        &mut self,
        ev: Option<io::Result<TermEvent>>,
    ) -> io::Result<Option<End>> {
        let now = Instant::now();
        match ev {
            Some(Ok(TermEvent::Key(k))) => {
                let Some(k) = key(k) else { return Ok(None) };
                let effects = self.driver.key(k, now);
                if let Some(end) = self.act(effects).await? {
                    return Ok(Some(end));
                }
            }
            Some(Ok(TermEvent::Mouse(m))) => return self.on_mouse(m, now).await,
            Some(Ok(TermEvent::Resize(..))) => {}
            Some(Ok(TermEvent::FocusGained)) => self.driver.panel.on_terminal_focus(true),
            Some(Ok(TermEvent::FocusLost)) => self.driver.panel.on_terminal_focus(false),
            Some(Ok(_)) => return Ok(None),
            Some(Err(e)) => return Err(e),
            None => return Ok(Some(End::Input)),
        }
        self.repaint(Painted::Other).map(|()| None)
    }

    async fn on_mouse(&mut self, m: MouseEvent, now: Instant) -> io::Result<Option<End>> {
        let what = match m.kind {
            MouseEventKind::Down(MouseButton::Left) => Pointer::Click,
            MouseEventKind::Moved | MouseEventKind::Drag(_) => Pointer::Move,
            MouseEventKind::ScrollUp => Pointer::WheelUp,
            MouseEventKind::ScrollDown => Pointer::WheelDown,
            _ => return Ok(None),
        };
        let (effects, repaint) =
            self.driver
                .pointer(what, i32::from(m.column), i32::from(m.row), now);
        if let Some(end) = self.act(effects).await? {
            return Ok(Some(end));
        }
        match repaint {
            Some(cause) => self.repaint(cause).map(|()| None),
            None => Ok(None),
        }
    }
}
