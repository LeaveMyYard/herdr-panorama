//! The terminal (plan §2.6): ratatui over crossterm, which sends only the cells that changed.
//! A [`Screen`] is copied into the frame's buffer at the edge. The terminal is set up for the
//! panel (alternate screen, raw mode, any-motion SGR mouse, focus reports) and always put back,
//! on a panic too.

use panorama_core::view::palette::BG;
use panorama_core::view::{Rgb, Screen};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier};
use std::io::{self, Stdout};

/// The panel's terminal.
pub type Term = ratatui::Terminal<CrosstermBackend<Stdout>>;

fn colour(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// Copies `s` into `buf` (a cell without a background gets panorama's, as Python's grid
/// paints it).
pub fn copy(s: &Screen, buf: &mut Buffer) {
    let area = buf.area;
    for y in 0..s.h().min(i32::from(area.height)) {
        for x in 0..s.w().min(i32::from(area.width)) {
            let c = s.cell(x, y);
            let pos = Position::new(area.x + x as u16, area.y + y as u16);
            if let Some(cell) = buf.cell_mut(pos) {
                cell.reset();
                cell.set_char(c.ch)
                    .set_fg(colour(c.fg))
                    .set_bg(colour(c.bg.unwrap_or(BG)));
                if c.bold {
                    cell.modifier.insert(Modifier::BOLD);
                }
            }
        }
    }
}

/// Puts the terminal back (idempotent; used on exit and from the panic hook).
pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableFocusChange,
        DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = execute!(io::stdout(), ratatui::crossterm::cursor::Show);
}

/// Whether the terminal is gone (its other side closed): crossterm's reader never reports
/// it (it reads the error again and again), and without a controlling terminal no SIGHUP
/// comes either.
pub fn hung_up() -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let stdin = io::stdin();
    let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
    let now = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    match poll(&mut fds, Some(&now)) {
        Ok(_) => fds[0]
            .revents()
            .intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL),
        Err(_) => false,
    }
}

/// Sets the terminal up for the panel.
pub fn setup() -> io::Result<Term> {
    enable_raw_mode()?;
    // the screen is cleared directly: ratatui's `clear` asks the terminal where its cursor is,
    // and a pane that never answers would make the panel fail at start
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange,
        Clear(ClearType::All)
    )?;
    let mut term = ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))?;
    term.hide_cursor()?;
    Ok(term)
}

#[cfg(test)]
mod tests;
