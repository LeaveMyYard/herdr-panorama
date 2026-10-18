//! The terminal's events: keys, the mouse, resizes, focus in and out (Python `SwarmApp`'s
//! bindings and `Grid`'s mouse handlers).

use super::{Actor, End};
use crate::panel::input::keys::key;
use crate::panel::input::mouse::WHEEL_ROWS;
use crate::panel::input::{Effect, Painted};
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
            Some(Ok(TermEvent::Key(k))) => match key(k) {
                Some(k) => {
                    let effects = self.panel.on_key(k, now);
                    if let Some(end) = self.act(effects).await? {
                        return Ok(Some(end));
                    }
                }
                None => return Ok(None),
            },
            Some(Ok(TermEvent::Mouse(m))) => return self.on_mouse(m, now).await,
            Some(Ok(TermEvent::Resize(..))) => {}
            Some(Ok(TermEvent::FocusGained)) => self.panel.on_terminal_focus(true),
            Some(Ok(TermEvent::FocusLost)) => self.panel.on_terminal_focus(false),
            Some(Ok(_)) => return Ok(None),
            Some(Err(e)) => return Err(e),
            None => return Ok(Some(End::Input)),
        }
        self.repaint(Painted::Other).map(|()| None)
    }

    /// A click acts on what was drawn under it (a notice there is dismissed instead); the
    /// pointer moving moves the hover look; the wheel scrolls the column under it.
    async fn on_mouse(&mut self, m: MouseEvent, now: Instant) -> io::Result<Option<End>> {
        let (x, y) = (i32::from(m.column), i32::from(m.row));
        let Some(shown) = self.shown.take() else {
            return Ok(None);
        };
        let (effects, cause, changed): (Vec<Effect>, Painted, bool) = match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.toasts.dismiss_at(x, y, &self.toast_rects) {
                    (vec![], Painted::Other, true)
                } else {
                    (self.panel.click(x, y, &shown, now), Painted::Other, true)
                }
            }
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                let changed = self.panel.hover(Some((x, y)), &shown);
                (vec![], Painted::Other, changed)
            }
            MouseEventKind::ScrollDown => (
                vec![],
                Painted::Wheel,
                self.panel.wheel(x, y, WHEEL_ROWS, &shown),
            ),
            MouseEventKind::ScrollUp => (
                vec![],
                Painted::Wheel,
                self.panel.wheel(x, y, -WHEEL_ROWS, &shown),
            ),
            _ => (vec![], Painted::Other, false),
        };
        self.shown = Some(shown);
        if let Some(end) = self.act(effects).await? {
            return Ok(Some(end));
        }
        if changed {
            self.repaint(cause)?;
        }
        Ok(None)
    }
}
