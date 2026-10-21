//! `panorama panel`: a per-tab panel viewer (docs/DESIGN.md "Per-tab panels"; Python
//! `panel.py`). It shows its own tab's docked panel (design 9e, turn 10's focus look),
//! drawn from the core's published state; it collects nothing and follows nothing.
//!
//! Without a core it keeps its last picture, dimmed, with the operator's notice
//! "panorama is restarting…" (Q139), reconnects, and exits after 10 minutes without one.
//! The core's `shutdown` ends it (herdr closes the pane with it). An error leaves the crash
//! marker the core looks for.
//!
//! - [`model`]: the panel's state as the core's messages change it;
//! - [`paint`]: a frame of it;
//! - [`actor`]: the event loop; the terminal, the clocks and the crash marker are
//!   [`crate::ui`]'s, shared with the app.

pub mod act;
pub mod actor;
pub mod input;
pub mod model;
pub mod no_core;
pub mod paint;

use crate::proto::client::{BACKOFF, Client, Config};
use crate::proto::socket;
use crate::settings::Settings;
use crate::ui::{clock, crash, term};
use actor::Actor;
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::internal::plugin_paths::panorama::{session_key, state_dir};
use herdr_agent_kit::plugin_env::PluginEnv;
use model::Panel;
use std::path::Path;
use tokio::sync::mpsc;

/// Runs the panel until it ends; the process's exit code.
pub fn run() -> i32 {
    let env = PluginEnv::from_env();
    let state = state_dir(&env);
    let pane = env.pane_id().map(str::to_owned);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(crash::hook(
        state.clone(),
        pane.clone(),
        term::restore,
        default_hook,
    ));
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .and_then(|rt| rt.block_on(serve(&env)));
    term::restore();
    match result {
        Ok(_) => 0,
        Err(e) => {
            crash::mark(
                &state,
                pane.as_deref(),
                &format!("panorama panel: {e}"),
                &clock::stamp(),
            );
            1
        }
    }
}

async fn serve(env: &PluginEnv) -> std::io::Result<actor::End> {
    let cfg = Config {
        path: socket::path(env),
        uid: env.uid(),
        pane_id: env.pane_id().map(str::to_owned),
        tab_id: env.tab_id().map(str::to_owned),
        backoff: BACKOFF.to_vec(),
    };
    let panel = Panel::new(cfg.pane_id.clone(), cfg.tab_id.clone());
    let (tx, rx) = mpsc::unbounded_channel();
    let client = Client::spawn(cfg, tx);
    let place = actor::Place {
        plugin_id: env.plugin_id(PANORAMA_ID).to_owned(),
        root: env.plugin_root().map(Path::to_path_buf),
        new_agent_mark: act::new_agent::marker(&state_dir(env), &session_key(env)),
        home: env
            .home()
            .map_or_else(|| "~".to_owned(), |h| h.to_string_lossy().into_owned()),
    };
    let herdr = crate::trace::client();
    let term = term::setup()?;
    let mut actor = Actor::new(
        panel,
        Settings::from_env(env),
        term,
        rx,
        client.clone(),
        herdr,
        place,
    );
    let end = actor.run().await;
    client.stop().await;
    end
}
