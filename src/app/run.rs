//! `panorama tui` (Python `cli.cmd_tui`): the graph popup, or the home tab's view with
//! `PANORAMA_HOME=1`, opened on the pane herdr says was focused.

use super::actor::Actor;
use super::collect::{Refresher, Worker};
use super::driver::Driver;
use super::link::Link;
use super::model::App;
use super::panic;
use crate::produce::Producer;
use crate::settings::Settings;
use crate::snapshot::SelfPane;
use crate::ui::end::End;
use crate::ui::term;
use herdr_agent_kit::internal::plugin_paths::panorama::state_dir;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::Value;
use std::sync::{Arc, Mutex};
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
    let self_pane: SelfPane = Arc::new(Mutex::new(
        home_mode(&env)
            .then(|| env.pane_id().map(str::to_owned))
            .flatten(),
    ));
    let pane = self_pane.clone();
    run_with(
        move || Producer::from_env_shared(PluginEnv::from_env(), pane),
        self_pane,
    )
}

/// The home tab's view (`PANORAMA_HOME=1`, Python `home.is_home`).
fn home_mode(env: &PluginEnv) -> bool {
    env.var("PANORAMA_HOME") == Some("1")
}

/// Runs the app until it ends, refreshed by `make`'s refresher; the exit code.
pub fn run<R: Refresher + 'static>(make: impl FnOnce() -> R + Send + 'static) -> i32 {
    run_with(make, SelfPane::default())
}

/// [`run`], the view's own pane shared with the collector (the home view moves it).
pub fn run_with<R: Refresher + 'static>(
    make: impl FnOnce() -> R + Send + 'static,
    self_pane: SelfPane,
) -> i32 {
    let env = PluginEnv::from_env();
    let log = panic::log_path(&state_dir(&env));
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(panic::hook(log.clone(), term::restore, default_hook));
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .and_then(|rt| rt.block_on(serve(&env, make, self_pane)));
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
    self_pane: SelfPane,
) -> std::io::Result<End> {
    let focus = invocation_pane(env.plugin_context_json());
    let app = App::new(focus, home_mode(env), Instant::now());
    let (tx, rx) = mpsc::unbounded_channel();
    let worker = Worker::spawn(make, tx)?;
    let home = home_mode(env);
    let me = home.then(|| env.pane_id().map(str::to_owned)).flatten();
    let log = panic::log_path(&state_dir(env));
    let mut link = Link::start(env, home, me, log.clone()).await;
    link.collector_pane = Some(self_pane);
    let term = term::setup()?;
    let mut actor = Actor::new(Driver::new(app), Settings::from_env(env), term, worker, rx)
        .with_link(link, log);
    let end = actor.run().await;
    actor.stop(STOP_WAIT); // then the thread is left behind: the process exits
    if let Some(a) = actor.after_exit.take() {
        // the inbox popup once this one is closed (Python `after_exit`)
        term::restore();
        let h = crate::trace::client();
        crate::panel::act::inbox::InboxLink::default()
            .open(
                &h,
                &crate::panel::act::inbox::UvRunner,
                a.question,
                a.pane.as_deref(),
            )
            .await;
    }
    end
}

#[cfg(test)]
mod tests;
