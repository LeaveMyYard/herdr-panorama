//! The app's one actor (plan §2.5; Python `SwarmApp`'s timers and handlers): every source of
//! events sends into one `select!`, and only the actor changes the app and draws.
//!
//! - every 1.5 s a collect is asked for (one at a time, [`super::collect`]); its swarm
//!   refreshes the model, its failure says so in the header;
//! - the terminal: keys and the pointer go to the [`Driver`] ([`terminal`]); resizes and focus
//!   in and out repaint;
//! - the colour tick (`animation_fps`): a new frame only while something moves and the frame
//!   differs ([`crate::ui::pacing`]); a notice's time running out;
//! - SIGTERM and SIGHUP end it; so does a terminal that hung up (looked at every second).
//!
//! The effects the input asks for go out from [`effects`]; drawing is [`draw`]'s.

mod draw;
mod effects;
mod terminal;

use super::collect::{Collected, Worker};
use super::driver::Driver;
use crate::settings::Settings;
use crate::ui::end::End;
use crate::ui::frame::Frame;
use crate::ui::input::Painted;
use crate::ui::pacing::{Pacing, ticking};
use crate::ui::term::{self, HANGUP_CHECK, Term, hung_up_error};
use crossterm::event::EventStream;
use futures_util::StreamExt;
use std::io;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

/// How often a collect is asked for (Python `REFRESH_SECONDS`).
pub const REFRESH: Duration = Duration::from_millis(1500);

/// The actor's state.
pub struct Actor {
    driver: Driver,
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
        driver: Driver,
        settings: Settings,
        term: Term,
        worker: Worker,
        collected: mpsc::UnboundedReceiver<Collected>,
    ) -> Self {
        Self {
            driver,
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
        self.repaint(Painted::Other)?;
        loop {
            let toast_at = self.driver.toasts.next_expiry();
            let end = tokio::select! {
                _ = refresh.tick() => {
                    self.worker.request();
                    None
                }
                Some(c) = self.collected.recv() => self.on_collected(c).map(|()| None)?,
                ev = input.next() => self.on_terminal(ev)?,
                _ = tick.tick(), if self.moving() => self.animate().map(|()| None)?,
                () = sleep_until(toast_at), if toast_at.is_some() => {
                    self.driver.toasts.expire(Instant::now());
                    self.repaint(Painted::Other).map(|()| None)?
                }
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
    /// or the header says why it failed; the cells whose click target moved wait.
    fn on_collected(&mut self, c: Collected) -> io::Result<()> {
        self.worker.done();
        self.driver.collected(c, Instant::now());
        self.repaint(Painted::Refresh)
    }

    /// The app ended: the refresher stops, waited for at most `within`.
    pub fn stop(&self, within: Duration) -> bool {
        self.worker.stop(within)
    }

    /// Whether the tick has anything to do (a terminal view is always shown).
    fn moving(&self) -> bool {
        let app = &self.driver.app;
        ticking(true, &app.graph.swarm, &app.graph.view.unseen)
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at.into()).await,
        None => std::future::pending().await,
    }
}
