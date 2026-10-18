//! The app's one actor (plan §2.5; Python `SwarmApp`'s timers and handlers): every source of
//! events sends into one `select!`, and only the actor changes the app and draws.
//!
//! - every 1.5 s a collect is asked for (one at a time, [`super::collect`]); its swarm
//!   refreshes the model, its failure says so in the header;
//! - the terminal: resizes and focus in and out repaint; `q` (and Esc in a popup) ends it;
//! - the colour tick (`animation_fps`): a new frame only while something moves and the frame
//!   differs ([`crate::ui::pacing`]);
//! - SIGTERM and SIGHUP end it; so does a terminal that hung up (looked at every second).
//!
//! The app's other keys, the mouse and the herdr side (goto, the inbox, the panels and the
//! core) join in P4 8b and 8c.

use super::collect::{Collected, Worker};
use super::model::App;
use crate::ui::end::End;
use crate::ui::term::{HANGUP_CHECK, hung_up_error};
use crate::panel::input::keys::key;
use crate::settings::Settings;
use crate::ui::clock;
use crate::ui::frame::Frame;
use crate::ui::pacing::{Pacing, ticking};
use crate::ui::term::{self, Term};
use crossterm::event::{Event as TermEvent, EventStream};
use futures_util::StreamExt;
use panorama_core::view::Marks;
use std::io;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

/// How often a collect is asked for (Python `REFRESH_SECONDS`).
pub const REFRESH: Duration = Duration::from_millis(1500);

/// The actor's state.
pub struct Actor {
    app: App,
    settings: Settings,
    /// Never dropped: ratatui's drop writes to the terminal and panics when it is gone;
    /// `term::restore` puts it back instead.
    term: ManuallyDrop<Term>,
    worker: Worker,
    collected: mpsc::UnboundedReceiver<Collected>,
    look: Option<Frame>,
    pacing: Pacing,
}

impl Actor {
    pub fn new(
        app: App,
        settings: Settings,
        term: Term,
        worker: Worker,
        collected: mpsc::UnboundedReceiver<Collected>,
    ) -> Self {
        Self {
            app,
            settings,
            term: ManuallyDrop::new(term),
            worker,
            collected,
            look: None,
            pacing: Pacing::default(),
        }
    }

    /// Runs until the app ends; a terminal that hung up is an end, not an error.
    pub async fn run(&mut self) -> io::Result<End> {
        match self.serve().await {
            Err(e) if hung_up_error(&e) => Ok(End::Hangup),
            r => r,
        }
    }

    async fn serve(&mut self) -> io::Result<End> {
        let period = Duration::from_secs_f64(1.0 / self.settings.read().animation_fps);
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut refresh = tokio::time::interval(REFRESH);
        refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut hangup = tokio::time::interval(HANGUP_CHECK);
        let mut input = EventStream::new();
        let (mut term_sig, mut hup) = (
            signal(SignalKind::terminate())?,
            signal(SignalKind::hangup())?,
        );
        self.repaint()?;
        loop {
            let end = tokio::select! {
                _ = refresh.tick() => {
                    self.worker.request();
                    None
                }
                Some(c) = self.collected.recv() => self.on_collected(c).map(|()| None)?,
                ev = input.next() => self.on_terminal(ev)?,
                _ = tick.tick(), if self.moving() => self.animate().map(|()| None)?,
                _ = hangup.tick() => term::hung_up().then_some(End::Hangup),
                _ = term_sig.recv() => Some(End::Signal),
                _ = hup.recv() => Some(End::Signal),
            };
            if let Some(end) = end {
                return Ok(end);
            }
        }
    }

    /// A collect's outcome: the model refreshes (the unseen marks join with their tracker),
    /// or the header says why it failed.
    fn on_collected(&mut self, c: Collected) -> io::Result<()> {
        self.worker.done();
        match c {
            Collected::Swarm(swarm) => self.app.apply(swarm, Marks::new(), Instant::now()),
            Collected::Failed(why) => self.app.failed(&why),
        }
        self.repaint()
    }

    /// A terminal event; `Some` when the app ends.
    fn on_terminal(&mut self, ev: Option<io::Result<TermEvent>>) -> io::Result<Option<End>> {
        match ev {
            Some(Ok(TermEvent::Key(k))) => {
                if key(k).is_some_and(|k| self.app.quits(k)) {
                    return Ok(Some(End::Quit));
                }
                return Ok(None);
            }
            Some(Ok(TermEvent::Resize(..))) => {}
            Some(Ok(TermEvent::FocusGained)) => self.app.pane_focused = true,
            Some(Ok(TermEvent::FocusLost)) => self.app.pane_focused = false,
            Some(Ok(_)) => return Ok(None),
            Some(Err(e)) => return Err(e),
            None => return Ok(Some(End::Input)),
        }
        self.repaint().map(|()| None)
    }

    /// Whether the tick has anything to do (a terminal view is always shown).
    fn moving(&self) -> bool {
        ticking(true, &self.app.graph.swarm, &self.app.graph.view.unseen)
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

    /// The colour tick: a new frame only when it would differ.
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
        let (w, h) = (i32::from(size.width), i32::from(size.height));
        let Some(screen) = self.app.paint(w, h, look) else {
            return Ok(());
        };
        self.pacing.drawn(look.frame, &screen);
        self.term.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        Ok(())
    }
}
