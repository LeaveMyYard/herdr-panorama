//! Drawing: a full repaint (the settings read again), or an animation frame; the notices go
//! over the view, and each paint feeds the click arming.

use super::Actor;
use crate::panel::clock;
use crate::panel::input::Painted;
use crate::panel::paint::Frame;
use crate::panel::term;
use std::io;
use std::time::Instant;

impl Actor {
    /// A full repaint: the settings read again, the frame of the clock now. `cause`: what
    /// changed (a state or the wheel holds the cells whose click target moved).
    pub(super) fn repaint(&mut self, cause: Painted) -> io::Result<()> {
        let now = clock::now();
        let read = self.settings.read();
        self.look = Some(Frame {
            now,
            frame: clock::frame_at(now),
            utc_offset: clock::utc_offset(),
            theme: read.theme,
            settings: read.panel,
            open_key: read.open_key,
        });
        self.draw(now, cause)
    }

    /// The colour tick: a new frame only when it would differ. A hidden tab gets none (its
    /// next state repaints it at the clock's frame, not where it paused).
    pub(super) fn animate(&mut self) -> io::Result<()> {
        let now = clock::now();
        if !self.pacing.due(clock::frame_at(now)) {
            return Ok(());
        }
        self.draw(now, Painted::Other)
    }

    fn draw(&mut self, now: f64, cause: Painted) -> io::Result<()> {
        let Some(look) = self.look.as_mut() else {
            return Ok(());
        };
        look.now = now;
        look.frame = clock::frame_at(now);
        let size = self.term.size()?;
        let view = self
            .panel
            .paint(i32::from(size.width), i32::from(size.height), look);
        self.pacing.drawn(look.frame, &view);
        self.panel
            .painted(self.shown.as_ref(), &view, cause, Instant::now());
        let mut screen = view.clone();
        self.toast_rects = self.toasts.draw(&mut screen);
        self.shown = Some(view);
        self.term.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        Ok(())
    }
}
