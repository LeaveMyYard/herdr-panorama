//! Core-only collection uses the same transcript collector without any TUI API calls.
use super::{
    Core,
    source::{CoreSource, SnapshotSource},
};
use crate::{
    collect::{
        Collector, Options,
        paths::{events_dir, hooks_dir},
    },
    settings::durations::{done_window, stale_after},
};
use herdr_agent_kit::{
    agent_session::claude::io::ProjectsRoot,
    internal::{inbox_contract::sqlite::open_questions, plugin_paths::inbox},
    plugin_env::{ConfigFile, PluginEnv},
};
use panorama_core::model::Swarm;
use serde_json::{Map, Value};

pub fn source(env: PluginEnv, core: Core) -> impl FnMut() -> Swarm + Send {
    source_mode(env, core, false)
}

pub(crate) fn source_mode(env: PluginEnv, core: Core, wait: bool) -> impl FnMut() -> Swarm + Send {
    let mut host = CoreSource {
        core: core.clone(),
        env: env.clone(),
        hooks_dir: hooks_dir(&env),
    };
    let mut config = ConfigFile::resolve(&env, || None);
    let projects = ProjectsRoot::from_env()
        .map_or_else(|| "~/.claude/projects".into(), |r| r.path().to_path_buf());
    let mut collector = Collector::new(Options {
        projects,
        events_dir: events_dir(&env),
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
        if wait {
            crate::snapshot::warmup::collect(refresh)
        } else {
            refresh().0
        }
    }
}
