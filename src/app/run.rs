//! `panorama tui` (Python `cli.cmd_tui`): the graph popup, or the home tab's view with
//! `PANORAMA_HOME=1`, opened on the pane herdr says was focused.

use super::actor::Actor;
use super::collect::{Refresher, Worker};
use super::driver::Driver;
use super::model::App;
use super::panic;
use crate::produce::Producer;
use crate::settings::Settings;
use crate::ui::end::End;
use crate::ui::term;
use herdr_agent_kit::internal::plugin_paths::panorama::state_dir;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// The pane that was focused when herdr invoked the plugin (Python `invocation_pane`).
pub fn invocation_pane(context_json: Option<&str>) -> Option<String> {
    let ctx: Value = serde_json::from_str(context_json?).ok()?;
    ctx.get("focused_pane_id")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
}

/// How long quitting waits for the refresher to stop. Its remote watchers are halted at once
/// and a sync under way is killed within the rsync runner's poll, well inside this; a local
/// refresh under way, or a forwarded snapshot's `herdr --machine … api snapshot` (read-only,
/// at most 8 s), is never waited out and may outlive the exit (Python quits at once).
pub const STOP_WAIT: Duration = Duration::from_millis(500);

/// `panorama tui`: the core's producer (this machine's swarm, herdr's saved machines, the
/// unseen marks) on the refresh thread. In the home tab the view's own pane is the one the
/// collector docks by (Python `collector.self_pane`). The exit code.
pub fn tui() -> i32 {
    let env = PluginEnv::from_env();
    let self_pane = home_mode(&env)
        .then(|| env.pane_id().map(str::to_owned))
        .flatten();
    run(move || Producer::from_env(PluginEnv::from_env(), self_pane))
}

/// The home tab's view (`PANORAMA_HOME=1`, Python `home.is_home`).
fn home_mode(env: &PluginEnv) -> bool {
    env.var("PANORAMA_HOME") == Some("1")
}

/// Runs the app until it ends, refreshed by `make`'s refresher; the exit code.
pub fn run<R: Refresher + 'static>(make: impl FnOnce() -> R + Send + 'static) -> i32 {
    let env = PluginEnv::from_env();
    let log = panic::log_path(&state_dir(&env));
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(panic::hook(log.clone(), term::restore, default_hook));
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .and_then(|rt| rt.block_on(serve(&env, make)));
    term::restore();
    match result {
        Ok(_) => 0,
        Err(e) => {
            panic::log(&log, &format!("panorama tui: {e}"));
            eprintln!("panorama tui: {e}");
            1
        }
    }
}

async fn serve<R: Refresher + 'static>(
    env: &PluginEnv,
    make: impl FnOnce() -> R + Send + 'static,
) -> std::io::Result<End> {
    let focus = invocation_pane(env.plugin_context_json());
    let app = App::new(focus, home_mode(env), Instant::now());
    let (tx, rx) = mpsc::unbounded_channel();
    let worker = Worker::spawn(make, tx)?;
    let term = term::setup()?;
    let mut actor = Actor::new(Driver::new(app), Settings::from_env(env), term, worker, rx);
    let end = actor.run().await;
    actor.stop(STOP_WAIT); // then the thread is left behind: the process exits
    end
}

#[cfg(test)]
mod tests;
