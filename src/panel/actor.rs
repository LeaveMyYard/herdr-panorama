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
use super::no_core::{NO_CORE_CHECK, NoCoreClock};
use super::pacing::{Pacing, ticking};
use super::paint::Frame;
use super::term::{self, Term};
use crate::proto::client::Event;
use crate::settings::Settings;
use crossterm::event::{Event as TermEvent, EventStream};
use futures_util::StreamExt;
use std::io;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

/// Why the panel ended (a clean end: herdr closes the pane with the viewer).
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    /// The core said the panels go.
    Shutdown,
    /// No core for `no_core::NO_CORE`.
    NoCore,
    /// A signal (the pane closed, or the process was asked to stop).
    Signal,
    /// The terminal's input ended.
    Input,
    /// The core client stopped on its own (its task ended: a panic in it, say).
    ClientGone,
}

pub struct Actor {
    panel: Panel,
    settings: Settings,
    term: Term,
    events: mpsc::UnboundedReceiver<Event>,
    /// The look and the open key, as last read (on every full repaint, as Python does).
    look: Option<Frame>,
    /// What is on screen, for the colour tick.
    pacing: Pacing,
    no_core: NoCoreClock,
}

impl Actor {
    pub fn new(
        panel: Panel,
        settings: Settings,
        term: Term,
        events: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            panel,
            settings,
            term,
            events,
            look: None,
            pacing: Pacing::default(),
            no_core: NoCoreClock::started(Instant::now()),
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
            let end = tokio::select! {
                ev = self.events.recv() => self.on_client(ev)?,
                ev = input.next() => self.on_terminal(ev)?,
                // a hidden tab, or a still screen, does not even wake for the tick
                _ = tick.tick(), if self.moving() => self.animate().map(|()| None)?,
                _ = no_core.tick() => self.no_core.expired(Instant::now()).then_some(End::NoCore),
                _ = term_sig.recv() => Some(End::Signal),
                _ = hup.recv() => Some(End::Signal),
            };
            if let Some(end) = end {
                return Ok(end);
            }
        }
    }

    /// The core client's event; `Some` when the panel ends.
    fn on_client(&mut self, ev: Option<Event>) -> io::Result<Option<End>> {
        match ev {
            Some(Event::State(st)) => self.panel.on_state(*st),
            Some(Event::Connected(on)) => {
                self.panel.on_connected(on);
                self.no_core.connected(on, Instant::now());
            }
            Some(Event::Shutdown) => return Ok(Some(End::Shutdown)),
            Some(Event::Refused(_)) => return Ok(None),
            None => return Ok(Some(End::ClientGone)),
        }
        self.repaint().map(|()| None)
    }

    /// A terminal event (keys and the mouse join with the panel's input); `Some` when the
    /// panel ends.
    fn on_terminal(&mut self, ev: Option<io::Result<TermEvent>>) -> io::Result<Option<End>> {
        match ev {
            Some(Ok(TermEvent::Resize(..))) => {}
            Some(Ok(TermEvent::FocusGained)) => self.panel.on_terminal_focus(true),
            Some(Ok(TermEvent::FocusLost)) => self.panel.on_terminal_focus(false),
            Some(Ok(_)) => return Ok(None),
            Some(Err(e)) => return Err(e),
            None => return Ok(Some(End::Input)),
        }
        self.repaint().map(|()| None)
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

    /// Whether the tick has anything to do (`pacing::ticking`).
    fn moving(&self) -> bool {
        ticking(
            self.panel.tab_shown,
            &self.panel.swarm,
            &self.panel.view.unseen,
        )
    }

    /// The colour tick: a new frame only when it would differ. A hidden tab gets none (its
    /// next state repaints it at the clock's frame, not where it paused).
    fn animate(&mut self) -> io::Result<()> {
        let now = clock::now();
        if !self.pacing.due(clock::frame_at(now)) {
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
        self.pacing.drawn(look.frame, &screen);
        self.term.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        Ok(())
    }
}
