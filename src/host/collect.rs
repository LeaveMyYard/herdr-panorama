//! Core-only collection uses the same transcript collector without any TUI API calls.
use super::{
    Core,
    source::{CoreSource, SnapshotSource},
};
use crate::{
    collect::{Collector, Options, paths::hooks_dir},
    settings::durations::{done_window, stale_after},
};
use herdr_agent_kit::{
    agent_session::claude::io::ProjectsRoot,
    internal::{inbox_contract::sqlite::open_questions, plugin_paths::inbox},
    plugin_env::{ConfigFile, PluginEnv},
};
use panorama_core::model::Swarm;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct Collected {
    pub pending: bool,
    pub swarm: Swarm,
    pub host: super::Host,
    pub terminals: HashMap<String, String>,
}

pub fn app_source(env: PluginEnv, core: Core) -> impl FnMut() -> Collected + Send {
    source_full(env, core, false)
}

pub fn source(env: PluginEnv, core: Core) -> impl FnMut() -> Swarm + Send {
    source_mode(env, core, false)
}

pub(crate) fn source_mode(env: PluginEnv, core: Core, wait: bool) -> impl FnMut() -> Swarm + Send {
    let mut source = source_full(env, core, wait);
    move || source().swarm
}

fn source_full(env: PluginEnv, core: Core, wait: bool) -> impl FnMut() -> Collected + Send {
    let projects: std::path::PathBuf = ProjectsRoot::from_env()
        .map_or_else(|| "~/.claude/projects".into(), |r| r.path().to_path_buf());
    let mut host = CoreSource {
        core: core.clone(),
        env: env.clone(),
        hooks_dir: hooks_dir(&env),
        link: super::link::Linker::system(&env, projects.clone()),
    };
    let mut config = ConfigFile::resolve(&env, || None);
    let mut collector = Collector::new(Options {
        projects,
        hooks_dir: hooks_dir(&env),
        procs: true,
        pane_pid: Box::new(move |pane| {
            let address = pane.strip_prefix("core:")?;
            let info = core.process(address)?;
            let pid = info["process_info"]["foreground_processes"][0]["pid"].as_u64()?;
            u32::try_from(pid).ok()
        }),
    })
    .with_core_source();
    if let Some(options) = crate::codex::Options::from_env(&env) {
        collector = collector.with_codex(options);
    }
    move || {
        let cfg: Map<String, Value> = config.load_as().unwrap_or_default();
        let observation = host.read();
        let questions = open_questions(&inbox::db_path(&env));
        let mut refresh = || {
            let mut swarm = collector.collect(
                observation.snapshot.as_ref(),
                questions.as_deref(),
                crate::snapshot::now(),
                done_window(env.var("PANORAMA_DONE_WINDOW"), &cfg),
                stale_after(env.var("PANORAMA_STALE_AFTER"), &cfg),
            );
            swarm.errors.extend(observation.errors.clone());
            let pending = collector.codex_pending(&swarm);
            (swarm, pending)
        };
        let swarm = if wait {
            crate::snapshot::warmup::collect(refresh)
        } else {
            refresh().0
        };
        let pending = collector.codex_pending(&swarm);
        Collected {
            pending,
            swarm,
            host: observation.host,
            terminals: observation.terminals,
        }
    }
}
