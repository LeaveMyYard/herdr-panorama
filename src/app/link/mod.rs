//! The home view's herdr side and its per-tab panels (Python `SwarmApp`'s workers, the core
//! and the panel manager): what the input asks for ([`acts`]), the core's state published on
//! every refresh ([`state`]), the panels' passes ([`panels`]), herdr's focus events
//! ([`events`]) and the core's pane check ([`verify`]). SAFETY: it focuses, opens and (through
//! the panel manager) closes panes. Everything that waits on herdr runs off the actor and
//! reports back as [`Done`].

pub mod acts;
pub mod done;
pub mod events;
mod focus;
pub mod panels;
pub mod state;
pub mod verify;

pub use done::Done;

use crate::core::{self, Config, Core, Event};
use crate::home::config::per_tab;
use crate::home::real::RealHome;
use crate::home::{self, Places};
use crate::panel::act::inbox::InboxLink;
use crate::panel::act::new_agent;
use crate::panel::actor::Place;
use crate::panels::real::RealOps;
use crate::panels::{FocusLog, Manager};
use crate::settings::Settings;
use herdr_agent_kit::herdr_api::Herdr;
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::internal::plugin_paths::panorama::{
    core_lock_path, core_socket_path, home_lock_path, session_key, state_dir,
};
use herdr_agent_kit::plugin_env::PluginEnv;
use panels::{PanelsLink, PanelsTask};
use panorama_core::model::Swarm;
use panorama_core::view::Filter;
use state::Published;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// An announced focus herdr has not confirmed by then is read again (Python
/// `FOCUS_CONFIRM_SECONDS`).
pub const FOCUS_CONFIRM: Duration = Duration::from_millis(750);
/// The longest quit waits for each of its steps (the panels, the home tab's label).
pub const QUIT_STEP: Duration = Duration::from_secs(5);

/// The view's own pane moved (herdr gave it a new id): the collector follows.
impl Link {
    pub fn moved(&mut self, pane: String) {
        if let Some(c) = &self.collector_pane {
            *c.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pane.clone());
        }
        self.my_pane = Some(pane);
    }
}

/// What reaches the actor from the herdr side.
#[derive(Debug)]
pub enum LinkEvent {
    Done(Done),
    Core(Event),
}

/// The home view's link to herdr, the core and the panels.
pub struct Link {
    herdr: Herdr,
    home_mode: bool,
    /// The view's own pane (the home pane).
    pub my_pane: Option<String>,
    /// The collector's copy of it, moved with it (Python `collector.self_pane`).
    pub collector_pane: Option<crate::snapshot::SelfPane>,
    place: Place,
    inbox: InboxLink,
    done_tx: mpsc::UnboundedSender<Done>,
    done_rx: mpsc::UnboundedReceiver<Done>,
    core: Option<Arc<Core>>,
    core_events: Option<mpsc::Receiver<Event>>,
    panels: Option<PanelsLink>,
    swarm: Arc<Mutex<Option<Swarm>>>,
    /// What the last state said besides the model.
    pub published: Published,
    /// The filter the last state carried.
    published_filter: Option<Filter>,
    /// A focus a panel announced, until herdr confirms one.
    unconfirmed: Option<String>,
    places: Places,
    log: PathBuf,
}

impl Link {
    /// The link for `env`'s view. The home view in per-tab mode starts the core (one per herdr
    /// session) and the panel manager; the home view also watches herdr's focus events.
    pub async fn start(
        env: &PluginEnv,
        home_mode: bool,
        my_pane: Option<String>,
        log: PathBuf,
    ) -> Self {
        let herdr = Herdr::from_env().with_client("panorama");
        let state = state_dir(env);
        let place = Place {
            plugin_id: env.plugin_id(PANORAMA_ID).to_owned(),
            root: env.plugin_root().map(Path::to_path_buf),
            new_agent_mark: new_agent::marker(&state, &session_key(env)),
            home: env
                .home()
                .map_or_else(|| "~".to_owned(), |h| h.to_string_lossy().into_owned()),
        };
        let mut link = Self::bare(
            herdr.clone(),
            home_mode,
            my_pane,
            place,
            Places::from_env(env),
            log,
        );
        if !home_mode {
            return link;
        }
        let mut settings = Settings::from_env(env);
        let focus_log = Arc::new(FocusLog::default());
        if per_tab(&settings.panorama_table()) {
            link.start_core(env, &state, &mut settings, focus_log.clone())
                .await;
        }
        let panels = link.panels.clone();
        events::spawn(herdr, focus_log, link.done_tx.clone(), move || {
            if let Some(p) = &panels {
                p.pass();
            }
        });
        link
    }

    /// A link without a core or panels (a popup, a test).
    pub fn bare(
        herdr: Herdr,
        home_mode: bool,
        my_pane: Option<String>,
        place: Place,
        places: Places,
        log: PathBuf,
    ) -> Self {
        let (done_tx, done_rx) = mpsc::unbounded_channel();
        Self {
            herdr,
            home_mode,
            my_pane,
            place,
            inbox: InboxLink::default(),
            done_tx,
            done_rx,
            collector_pane: None,
            core: None,
            core_events: None,
            panels: None,
            swarm: Arc::new(Mutex::new(None)),
            published: Published::default(),
            published_filter: None,
            unconfirmed: None,
            places,
            log,
        }
    }

    /// The next report from the herdr side, if one is already there (tests).
    pub fn try_done(&mut self) -> Option<Done> {
        self.done_rx.try_recv().ok()
    }

    async fn start_core(
        &mut self,
        env: &PluginEnv,
        state: &Path,
        settings: &mut Settings,
        focus_log: Arc<FocusLog>,
    ) {
        let cfg = Config::new(core_socket_path(env), core_lock_path(env), env.uid());
        let (tx, rx) = core::events();
        let core = match core::start(cfg, verify::HerdrVerify(self.herdr.clone()), tx).await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                crate::app::panic::log(&self.log, &format!("core: {e}"));
                return;
            }
        };
        let ops = RealOps {
            herdr: self.herdr.clone(),
            plugin_id: self.place.plugin_id.clone(),
            root: self.place.root.clone(),
            width: crate::panels::fit::width(settings.panorama_table().get("cockpit_width")),
        };
        let places = crate::panels::Places {
            state_path: Some(state.join(format!("panels-{}.json", session_key(env)))),
            marker_dir: Some(state.to_path_buf()), // the viewers write their crash markers here
            home_lock: home_lock_path(env),
        };
        let clock = Arc::new(std::time::Instant::now);
        let manager = Manager::new(ops, Some(focus_log), places, clock);
        let home = RealHome {
            herdr: self.herdr.clone(),
            plugin_id: self.place.plugin_id.clone(),
            root: self.place.root.clone(),
        };
        self.panels = Some(PanelsLink::spawn(PanelsTask {
            manager,
            core: core.clone(),
            swarm: self.swarm.clone(),
            settings: Settings::from_env(env),
            home,
            places: self.places.clone(),
            my_pane: self.my_pane.clone(),
            done: self.done_tx.clone(),
            log: self.log.clone(),
            #[cfg(test)]
            explode: Default::default(),
        }));
        self.core = Some(core);
        self.core_events = Some(rx);
    }

    /// The next thing from the herdr side (never ends while the view runs).
    pub async fn next(&mut self) -> LinkEvent {
        let core = async {
            match self.core_events.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            Some(d) = self.done_rx.recv() => LinkEvent::Done(d),
            Some(e) = core => LinkEvent::Core(e),
        }
    }

    /// The open key from the core's socket: answered from a task (it waits on herdr).
    pub fn open(&self, reply: tokio::sync::oneshot::Sender<String>) {
        let panels = self.panels.clone();
        tokio::spawn(async move {
            let did = match panels {
                Some(p) => p.open_key().await,
                None => "home".into(),
            };
            let _ = reply.send(did);
        });
    }

    /// Quitting: the panels go with panorama (they come back with it), then a home tab the
    /// operator split loses its label (issue #9).
    /// Each step is bounded (`QUIT_STEP`): a herdr call that hangs never holds the view open.
    pub async fn quit(&mut self) {
        if let Some(p) = &self.panels
            && tokio::time::timeout(QUIT_STEP, p.quit()).await.is_err()
        {
            crate::app::panic::log(&self.log, "panels: quit gave up waiting for herdr");
        }
        if self.home_mode
            && let Some(me) = self.my_pane.clone()
        {
            let home = RealHome {
                herdr: self.herdr.clone(),
                plugin_id: self.place.plugin_id.clone(),
                root: self.place.root.clone(),
            };
            let why = match tokio::time::timeout(QUIT_STEP, home::leave(&home, &self.places, &me))
                .await
            {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e),
                Err(_) => Some("herdr did not answer".into()),
            };
            if let Some(e) = why {
                crate::app::panic::log(&self.log, &format!("could not release the home tab: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests;
