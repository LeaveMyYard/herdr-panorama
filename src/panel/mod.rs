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
//! - [`actor`]: the event loop; [`term`]: the terminal; [`clock`]; [`crash`].

pub mod act;
pub mod actor;
pub mod clock;
pub mod crash;
pub mod input;
pub mod model;
pub mod no_core;
pub mod pacing;
pub mod paint;
pub mod term;
pub mod toast;

use crate::proto::client::{BACKOFF, Client, Config};
use crate::proto::socket;
use crate::settings::Settings;
use actor::Actor;
use herdr_agent_kit::internal::plugin_paths::panorama::state_dir;
use herdr_agent_kit::plugin_env::PluginEnv;
use model::Panel;
use tokio::sync::mpsc;

/// Runs the panel until it ends; the process's exit code.
pub fn run() -> i32 {
    let env = PluginEnv::from_env();
    let state = state_dir(&env);
    let pane = env.pane_id().map(str::to_owned);
    let hook_pane = pane.clone();
    let hook_state = state.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::restore();
        crash::mark(
            &hook_state,
            hook_pane.as_deref(),
            // the panic and where it happened, as Python's log keeps the traceback
            &format!("{info}\n{}", std::backtrace::Backtrace::force_capture()),
            &clock::stamp(),
        );
        default_hook(info);
    }));
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
    let term = term::setup()?;
    let mut actor = Actor::new(panel, Settings::from_env(env), term, rx);
    let end = actor.run().await;
    client.stop().await;
    end
}
