//! Drawing: a full repaint (the settings read again), or an animation frame; the notices go
//! over the view, and each paint feeds the click arming.

use super::Actor;
use crate::ui::clock;
use crate::ui::frame::Frame;
use crate::ui::input::Painted;
use crate::ui::term;
use std::io;
use std::time::Instant;

impl Actor {
    /// A full repaint: the settings read again, the frame of the clock now. `cause`: what
    /// changed (a refresh or the wheel holds the cells whose click target moved).
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

    /// The colour tick: a new frame only when it would differ.
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
        let (w, h) = (i32::from(size.width), i32::from(size.height));
        let Some(screen) = self.driver.paint(w, h, look, cause, Instant::now()) else {
            return Ok(());
        };
        self.pacing.drawn(look.frame, &screen);
        self.term.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        Ok(())
    }
}
