//! The panel's one actor (plan §2.5): every source of events sends into one `select!`, and
//! only the actor changes the panel and draws.
//!
//! - the core client's events: a state, connected or not, shutdown;
//! - the terminal: resizes and focus in/out (keys and the mouse join with the panel's input);
//! - the colour tick (`animation_fps`, default 20 a second): a new frame only while this tab
//!   is shown, something moves, and the frame differs (a new whole frame, or a colour that
//!   glides);
//! - every 10 s: no core for 10 minutes and the panel exits (herdr closes its pane);
//! - SIGTERM and SIGHUP: exit.

use super::clock;
use super::model::Panel;
use super::paint::Frame;
use super::term::{self, Term};
use crate::proto::client::{Client, Event};
use crate::settings::Settings;
use crossterm::event::{Event as TermEvent, EventStream};
use futures_util::StreamExt;
use panorama_core::view::moving::animated;
use std::io;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

/// No core this long: the viewer exits.
pub const NO_CORE: Duration = Duration::from_secs(600);
/// How often that is looked at.
const NO_CORE_CHECK: Duration = Duration::from_secs(10);

/// Why the panel ended (a clean end: herdr closes the pane with the viewer).
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    /// The core said the panels go.
    Shutdown,
    /// No core for [`NO_CORE`].
    NoCore,
    /// A signal (the pane closed, or the process was asked to stop).
    Signal,
    /// The terminal's input ended.
    Input,
    /// The core client stopped on its own.
    ClientGone,
}

pub struct Actor {
    pub panel: Panel,
    pub settings: Settings,
    pub term: Term,
    pub client: Client,
    pub events: mpsc::UnboundedReceiver<Event>,
    /// The look and the open key, as last read (on every full repaint, as Python does).
    look: Option<Frame>,
    /// The whole frame last drawn, and whether that screen had gliding colours.
    drawn_frame: Option<i64>,
    smooth: bool,
    no_core_since: Option<Instant>,
}

impl Actor {
    pub fn new(
        panel: Panel,
        settings: Settings,
        term: Term,
        client: Client,
        events: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            panel,
            settings,
            term,
            client,
            events,
            look: None,
            drawn_frame: None,
            smooth: false,
            no_core_since: Some(Instant::now()),
        }
    }

    /// Runs until the panel ends.
    pub async fn run(&mut self) -> io::Result<End> {
        // read when the view starts, as Python does
        let period = Duration::from_secs_f64(1.0 / self.settings.read().animation_fps);
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut no_core = tokio::time::interval(NO_CORE_CHECK);
        let mut input = EventStream::new();
        let (mut term_sig, mut hup) = (
            signal(SignalKind::terminate())?,
            signal(SignalKind::hangup())?,
        );
        self.repaint()?;
        loop {
            tokio::select! {
                ev = self.events.recv() => match ev {
                    Some(Event::State(st)) => {
                        self.panel.on_state(*st);
                        self.repaint()?;
                    }
                    Some(Event::Connected(on)) => {
                        self.panel.on_connected(on);
                        self.no_core_since = if on { None } else { self.no_core_since.or(Some(Instant::now())) };
                        self.repaint()?;
                    }
                    Some(Event::Shutdown) => return Ok(End::Shutdown),
                    Some(Event::Refused(_)) => {}
                    None => return Ok(End::ClientGone),
                },
                ev = input.next() => match ev {
                    Some(Ok(TermEvent::Resize(..))) => self.repaint()?,
                    Some(Ok(TermEvent::FocusGained)) => {
                        self.panel.on_terminal_focus(true);
                        self.repaint()?;
                    }
                    Some(Ok(TermEvent::FocusLost)) => {
                        self.panel.on_terminal_focus(false);
                        self.repaint()?;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e),
                    None => return Ok(End::Input),
                },
                // a hidden tab, or a still screen, does not even wake for the tick
                _ = tick.tick(), if self.moving() => self.animate()?,
                _ = no_core.tick() => {
                    if self.no_core_since.is_some_and(|t| t.elapsed() >= NO_CORE) {
                        return Ok(End::NoCore);
                    }
                }
                _ = term_sig.recv() => return Ok(End::Signal),
                _ = hup.recv() => return Ok(End::Signal),
            }
        }
    }

    /// A full repaint: the settings read again, the frame of the clock now.
    fn repaint(&mut self) -> io::Result<()> {
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
        self.draw(now)
    }

    /// Whether the tick has anything to do: this tab is shown and something on it moves.
    fn moving(&self) -> bool {
        self.panel.tab_shown && animated(&self.panel.swarm, &self.panel.view.unseen)
    }

    /// The colour tick: a new frame only when it would differ. A hidden tab gets none (its
    /// next state repaints it at the clock's frame, not where it paused).
    fn animate(&mut self) -> io::Result<()> {
        let now = clock::now();
        let whole = clock::frame_at(now).floor() as i64;
        if self.drawn_frame == Some(whole) && !self.smooth {
            return Ok(());
        }
        self.draw(now)
    }

    fn draw(&mut self, now: f64) -> io::Result<()> {
        let Some(look) = self.look.as_mut() else {
            return Ok(());
        };
        look.now = now;
        look.frame = clock::frame_at(now);
        let size = self.term.size()?;
        let screen = self
            .panel
            .paint(i32::from(size.width), i32::from(size.height), look);
        self.drawn_frame = Some(look.frame.floor() as i64);
        self.smooth = screen_moves(&screen);
        self.term.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        Ok(())
    }
}

/// Whether a screen has colours that glide between whole frames (pulses, blinks).
fn screen_moves(s: &panorama_core::view::Screen) -> bool {
    (0..s.h()).any(|y| {
        (0..s.w()).any(|x| {
            let c = s.cell(x, y);
            c.fg_moves || c.bg_moves
        })
    })
}
