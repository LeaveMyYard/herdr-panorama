//! The panel's one actor (plan §2.5): every source of events sends into one `select!`, and
//! only the actor changes the panel and draws.
//!
//! - the core client's events: a state, connected or not, shutdown ([`client`]);
//! - the terminal: keys, the mouse, resizes, focus in and out ([`terminal`]);
//! - the colour tick (`animation_fps`, default 20 a second): a new frame only while this tab
//!   is shown, something moves, and the frame differs ([`crate::ui::pacing`]);
//! - the arrows coming to rest, a notice's time running out;
//! - what the herdr side reports back ([`effects`]): a switch done, a pane checked;
//! - every 10 s: no core for 10 minutes and the panel exits ([`super::no_core`]);
//! - SIGTERM and SIGHUP: exit; a terminal that hung up (looked at every second): exit, cleanly.
//!
//! The decisions are the [`Driver`]'s; drawing ([`draw`]) puts the notices over the view and
//! feeds the click arming. A terminal that hung up (its pane closed) ends the panel cleanly.

mod announce;
mod client;
mod draw;
pub mod driver;
mod effects;
mod terminal;

pub use announce::Announcer;
pub use driver::Driver;
pub use effects::Done;

use super::input::Painted;
use super::no_core::{NO_CORE_CHECK, NoCoreClock};
use super::paint::Frame;
use crate::proto::client::{Client, Event};
use crate::settings::Settings;
use crate::trace::HerdrClient;
use crate::ui::pacing::{Pacing, ticking};
use crate::ui::term::Term;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use std::io;
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{mpsc, watch};

pub use crate::ui::end::End;
pub use crate::ui::term::{HANGUP_CHECK, hung_up_error as hung_up};

/// Where the panel's herdr side runs: its plugin, its root, the new-agent popup's mark, the
/// home directory (the popup's default directory).
#[derive(Clone, Debug)]
pub struct Place {
    pub plugin_id: String,
    pub root: Option<PathBuf>,
    pub new_agent_mark: PathBuf,
    pub home: String,
}

impl Place {
    /// The plugin's own: `HERDR_PLUGIN_ID`, `HERDR_PLUGIN_ROOT`, this session's mark, `$HOME`.
    pub fn from_env(env: &herdr_agent_kit::plugin_env::PluginEnv) -> Self {
        use herdr_agent_kit::internal::plugin_paths::{PANORAMA_ID, panorama};
        Self {
            plugin_id: env.plugin_id(PANORAMA_ID).to_owned(),
            root: env.plugin_root().map(std::path::Path::to_path_buf),
            new_agent_mark: super::act::new_agent::marker(
                &panorama::state_dir(env),
                &panorama::session_key(env),
            ),
            home: env
                .home()
                .map_or_else(|| "~".to_owned(), |h| h.to_string_lossy().into_owned()),
        }
    }
}

/// The actor's state.
pub struct Actor {
    driver: Driver,
    settings: Settings,
    /// Never dropped: ratatui's drop writes to the terminal and panics when it is gone (the
    /// pane closed); `term::restore` puts it back instead.
    term: ManuallyDrop<Term>,
    events: mpsc::UnboundedReceiver<Event>,
    client: Client,
    herdr: HerdrClient,
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
    arrow_at: Option<Instant>,
}

impl Actor {
    pub fn new(
        panel: super::model::Panel,
        settings: Settings,
        term: Term,
        events: mpsc::UnboundedReceiver<Event>,
        client: Client,
        herdr: HerdrClient,
        place: Place,
    ) -> Self {
        let (done_tx, done_rx) = mpsc::unbounded_channel();
        Self {
            driver: Driver::new(panel),
            settings,
            term: ManuallyDrop::new(term),
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
            arrow_at: None,
        }
    }

    /// Runs until the panel ends; a terminal that hung up is an end, not an error.
    pub async fn run(&mut self) -> io::Result<End> {
        match self.serve().await {
            Err(e) if hung_up(&e) => Ok(End::Hangup),
            r => r,
        }
    }

    async fn serve(&mut self) -> io::Result<End> {
        // read when the view starts, as Python does
        let period = Duration::from_secs_f64(1.0 / self.settings.read().animation_fps);
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut no_core = tokio::time::interval(NO_CORE_CHECK);
        let mut hangup = tokio::time::interval(HANGUP_CHECK);
        let mut input = EventStream::new();
        let (mut term_sig, mut hup) = (
            signal(SignalKind::terminate())?,
            signal(SignalKind::hangup())?,
        );
        self.repaint(Painted::Refresh)?;
        loop {
            let arrow_at = self.arrow_at;
            let toast_at = self.driver.toasts.next_expiry();
            let end = tokio::select! {
                ev = self.events.recv() => self.on_client(ev).await?,
                ev = input.next() => self.on_terminal(ev).await?,
                Some(done) = self.done_rx.recv() => self.on_done(done).await?,
                // a hidden tab, or a still screen, does not even wake for the tick
                _ = tick.tick(), if self.moving() => self.animate().map(|()| None)?,
                () = sleep_until(arrow_at), if arrow_at.is_some() => self.on_arrows_rest().await?,
                () = sleep_until(toast_at), if toast_at.is_some() => {
                    self.driver.toasts.expire(Instant::now());
                    self.repaint(Painted::Other).map(|()| None)?
                }
                _ = no_core.tick() => self.no_core.expired(Instant::now()).then_some(End::NoCore),
                _ = hangup.tick() => crate::ui::term::hung_up().then_some(End::Hangup),
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
        let p = &self.driver.panel;
        ticking(p.tab_shown, &p.swarm, &p.view.unseen)
    }

    /// The arrows came to rest: their switch goes out, and the marks they cleared go.
    async fn on_arrows_rest(&mut self) -> io::Result<Option<End>> {
        self.arrow_at = None;
        let effects = self.driver.panel.arrows_rested(Instant::now());
        if let Some(end) = self.act(effects).await? {
            return Ok(Some(end));
        }
        self.repaint(Painted::Other).map(|()| None)
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at.into()).await,
        None => std::future::pending().await,
    }
}
