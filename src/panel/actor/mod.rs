//! The panel's one actor (plan §2.5): every source of events sends into one `select!`, and
//! only the actor changes the panel and draws.
//!
//! - the core client's events: a state, connected or not, shutdown ([`client`]);
//! - the terminal: keys, the mouse, resizes, focus in and out ([`terminal`]);
//! - the colour tick (`animation_fps`, default 20 a second): a new frame only while this tab
//!   is shown, something moves, and the frame differs ([`super::pacing`]);
//! - the arrows coming to rest, a notice's time running out;
//! - what the herdr side reports back ([`effects`]): a switch done, a pane checked;
//! - every 10 s: no core for 10 minutes and the panel exits ([`super::no_core`]);
//! - SIGTERM and SIGHUP: exit.
//!
//! Drawing ([`draw`]) puts the notices over the view and feeds the click arming.

mod announce;
mod client;
mod draw;
mod effects;
mod terminal;

pub use announce::Announcer;
pub use effects::Done;

use super::input::Painted;
use super::no_core::{NO_CORE_CHECK, NoCoreClock};
use super::pacing::{Pacing, ticking};
use super::paint::Frame;
use super::term::Term;
use super::toast::{Placed, Toasts};
use crate::proto::client::{Client, Event};
use crate::settings::Settings;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use herdr_agent_kit::herdr_api::Herdr;
use panorama_core::view::Screen;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{mpsc, watch};

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
    /// The operator closed the panel (q).
    Quit,
}

/// Where the panel's herdr side runs: its plugin, its root, the new-agent popup's mark, the
/// home directory (the popup's default directory).
#[derive(Clone, Debug)]
pub struct Place {
    pub plugin_id: String,
    pub root: Option<PathBuf>,
    pub new_agent_mark: PathBuf,
    pub home: String,
}

/// The actor's state.
pub struct Actor {
    panel: super::model::Panel,
    settings: Settings,
    term: Term,
    events: mpsc::UnboundedReceiver<Event>,
    client: Client,
    herdr: Herdr,
    place: Place,
    inbox: super::act::inbox::InboxLink,
    /// The herdr side's reports, and the sender its tasks get.
    done_tx: mpsc::UnboundedSender<Done>,
    done_rx: mpsc::UnboundedReceiver<Done>,
    /// The focused pane of the latest state, for a switch's echo.
    focus: watch::Sender<Option<String>>,
    look: Option<Frame>,
    pacing: Pacing,
    no_core: NoCoreClock,
    toasts: Toasts,
    toast_rects: Vec<Placed>,
    /// The view as last drawn (its click targets), without the notices.
    shown: Option<Screen>,
    arrow_at: Option<Instant>,
}

impl Actor {
    pub fn new(
        panel: super::model::Panel,
        settings: Settings,
        term: Term,
        events: mpsc::UnboundedReceiver<Event>,
        client: Client,
        herdr: Herdr,
        place: Place,
    ) -> Self {
        let (done_tx, done_rx) = mpsc::unbounded_channel();
        Self {
            panel,
            settings,
            term,
            events,
            client,
            herdr,
            place,
            inbox: Default::default(),
            done_tx,
            done_rx,
            focus: watch::channel(None).0,
            look: None,
            pacing: Pacing::default(),
            no_core: NoCoreClock::started(Instant::now()),
            toasts: Toasts::default(),
            toast_rects: vec![],
            shown: None,
            arrow_at: None,
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
        self.repaint(Painted::Refresh)?;
        loop {
            let arrow_at = self.arrow_at;
            let toast_at = self.toasts.next_expiry();
            let end = tokio::select! {
                ev = self.events.recv() => self.on_client(ev).await?,
                ev = input.next() => self.on_terminal(ev).await?,
                Some(done) = self.done_rx.recv() => self.on_done(done).await?,
                // a hidden tab, or a still screen, does not even wake for the tick
                _ = tick.tick(), if self.moving() => self.animate().map(|()| None)?,
                () = sleep_until(arrow_at), if arrow_at.is_some() => self.on_arrows_rest().await?,
                () = sleep_until(toast_at), if toast_at.is_some() => {
                    self.toasts.expire(Instant::now());
                    self.repaint(Painted::Other).map(|()| None)?
                }
                _ = no_core.tick() => self.no_core.expired(Instant::now()).then_some(End::NoCore),
                _ = term_sig.recv() => Some(End::Signal),
                _ = hup.recv() => Some(End::Signal),
            };
            if let Some(end) = end {
                return Ok(end);
            }
        }
    }

    /// Whether the tick has anything to do (`pacing::ticking`).
    fn moving(&self) -> bool {
        ticking(
            self.panel.tab_shown,
            &self.panel.swarm,
            &self.panel.view.unseen,
        )
    }

    /// The arrows came to rest.
    async fn on_arrows_rest(&mut self) -> io::Result<Option<End>> {
        self.arrow_at = None;
        let effects = self.panel.arrows_rested(Instant::now());
        self.act(effects).await
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at.into()).await,
        None => std::future::pending().await,
    }
}
