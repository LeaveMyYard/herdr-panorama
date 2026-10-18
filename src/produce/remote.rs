//! The real remote machines for the producer (Python `machines.Remotes.start` with its
//! `saved()` and `Watcher`): the `machines` setting and herdr's saved machines from the CLI,
//! and per machine a watcher whose collector reads that machine's rsync mirror.

use crate::collect::mirror::MirrorCollector;
use crate::collect::paths::{events_dir, hooks_dir};
use crate::collect::{Collector, Options};
use crate::machines::sync::{RsyncSync, mirror_root};
use crate::machines::{Machine, Refresher, Remotes, Selection, Source, Watcher};
use crate::settings::durations::{done_window, stale_after};
use herdr_agent_kit::herdr_api::{Machine as Saved, blocking};
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::plugin_env::{ConfigFile, PluginEnv};
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex, PoisonError};

/// The setting from panorama's config, the machines from herdr's CLI.
struct HerdrMachines {
    config: ConfigFile,
    herdr: blocking::Herdr,
}

impl Source for HerdrMachines {
    fn selection(&mut self) -> Selection {
        Selection::from_config(
            &self
                .config
                .load_as::<Map<String, Value>>()
                .unwrap_or_default(),
        )
    }
    fn machines(&mut self) -> Vec<Saved> {
        self.herdr.machine_list().unwrap_or_default()
    }
}

/// The done window and stale time, read from the config when a machine's collect runs.
fn windows(env: PluginEnv, herdr: blocking::Herdr) -> Box<dyn FnMut() -> (f64, f64) + Send> {
    let mut config =
        ConfigFile::resolve(&env, || herdr.plugin_config_dir(PANORAMA_ID).ok().flatten());
    Box::new(move || {
        let cfg: Map<String, Value> = config.load_as().unwrap_or_default();
        (
            done_window(env.var("PANORAMA_DONE_WINDOW"), &cfg),
            stale_after(env.var("PANORAMA_STALE_AFTER"), &cfg),
        )
    })
}

/// Every selected machine's watcher, started: herdr's forwarded snapshot, the rsync mirror
/// (its done window from the config), the collector over the mirror (no process checks).
pub fn start(env: &PluginEnv, herdr: blocking::Herdr) -> Remotes {
    let config = ConfigFile::resolve(env, || herdr.plugin_config_dir(PANORAMA_ID).ok().flatten());
    let keep_window = Arc::new(Mutex::new(windows(env.clone(), herdr.clone())));
    let keep = {
        let w = keep_window.clone();
        Arc::new(move || (w.lock().unwrap_or_else(PoisonError::into_inner))().0)
            as Arc<dyn Fn() -> f64 + Send + Sync>
    };
    let syncer = Arc::new(RsyncSync::new(env, keep));
    let state = herdr_agent_kit::internal::plugin_paths::panorama::state_dir(env);
    let (env2, api) = (env.clone(), Arc::new(herdr.clone()));
    let spawn = move |m: Machine| {
        let collector = Collector::new(Options {
            projects: mirror_root(&state, &m),
            events_dir: events_dir(&env2),
            hooks_dir: hooks_dir(&env2),
            procs: false,
            pane_pid: Box::new(|_| None),
        });
        let collector = MirrorCollector {
            collector,
            windows: windows(env2.clone(), (*api).clone()),
            clock: Box::new(crate::snapshot::now),
        };
        Watcher::spawn(Refresher {
            machine: m,
            api: api.clone(),
            syncer: syncer.clone(),
            collector: Box::new(collector),
        })
    };
    Remotes::start(Box::new(HerdrMachines { config, herdr }), Box::new(spawn))
}

#[cfg(test)]
mod tests;
