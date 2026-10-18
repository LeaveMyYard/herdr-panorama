//! `panorama snapshot [--json | --counts]` (Python `cli.cmd_snapshot`, `cli.build`): one
//! collect of this machine's swarm (herdr's snapshot, the transcripts, the inbox), printed.
//! It never changes anything in herdr.

pub mod json;
pub mod print;
pub mod pyjson;

use crate::collect::liveness::claude_pid;
use crate::collect::paths::{events_dir, hooks_dir};
use crate::collect::{Collector, Options};
use crate::settings::durations::{done_window, stale_after};
use herdr_agent_kit::agent_session::claude::io::ProjectsRoot;
use herdr_agent_kit::herdr_api::blocking;
use herdr_agent_kit::internal::inbox_contract::sqlite::open_questions;
use herdr_agent_kit::internal::plugin_paths::{PANORAMA_ID, inbox};
use herdr_agent_kit::plugin_env::{ConfigFile, PluginEnv};
use panorama_core::model::Swarm;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How the swarm is printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The text listing.
    Text,
    /// The whole model as JSON.
    Json,
    /// The totals as JSON.
    Counts,
}

impl Format {
    /// From the arguments after `snapshot` (`--counts` wins over `--json`, as in Python);
    /// `None` for an argument it does not know.
    pub fn from_args(args: &[String]) -> Option<Self> {
        let mut f = Format::Text;
        for a in args {
            match a.as_str() {
                "--json" if f != Format::Counts => f = Format::Json,
                "--json" => {}
                "--counts" => f = Format::Counts,
                _ => return None,
            }
        }
        Some(f)
    }
}

/// herdr's live session snapshot, or `None` when herdr cannot give one.
pub fn herdr_snapshot(herdr: &blocking::Herdr) -> Option<Value> {
    let res = herdr.raw("session.snapshot", json!({})).ok()?;
    res.get("snapshot").filter(|s| s.is_object()).cloned()
}

/// POSIX seconds now.
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// One collect of this machine's swarm, as the CLI builds it.
pub fn build(env: &PluginEnv) -> Swarm {
    let herdr = blocking::Herdr::from_env().with_client("panorama");
    let mut config =
        ConfigFile::resolve(env, || herdr.plugin_config_dir(PANORAMA_ID).ok().flatten());
    let cfg: Map<String, Value> = config.load_as().unwrap_or_default();
    // no home at all: a relative "~/.claude/projects", which holds nothing (as Python's
    // expanduser leaves "~" when it finds no home)
    let projects = ProjectsRoot::from_env().map_or_else(
        || PathBuf::from("~/.claude/projects"),
        |r| r.path().to_path_buf(),
    );
    let lookup = herdr.clone();
    let mut collector = Collector::new(Options {
        projects,
        events_dir: events_dir(env),
        hooks_dir: hooks_dir(env),
        procs: true,
        pane_pid: Box::new(move |pane| claude_pid(&lookup, pane)),
    });
    let snapshot = herdr_snapshot(&herdr);
    let questions = open_questions(&inbox::db_path(env));
    let window = done_window(env.var("PANORAMA_DONE_WINDOW"), &cfg);
    let stale = stale_after(env.var("PANORAMA_STALE_AFTER"), &cfg);
    collector.collect(
        snapshot.as_ref(),
        questions.as_deref(),
        now(),
        window,
        stale,
    )
}

/// The text `panorama snapshot` prints in `format`, after `seconds` of collecting.
pub fn render(swarm: &Swarm, format: Format, seconds: f64) -> String {
    match format {
        Format::Text => print::listing(swarm),
        Format::Json => pyjson::dumps(&json::swarm(swarm), 2) + "\n",
        Format::Counts => print::counts(swarm, seconds) + "\n",
    }
}

/// `panorama snapshot <args>`; the exit status.
pub fn run(args: &[String]) -> i32 {
    let Some(format) = Format::from_args(args) else {
        eprintln!("usage: panorama snapshot [--json] [--counts]");
        return 2;
    };
    let started = Instant::now();
    let swarm = build(&PluginEnv::from_env());
    print!(
        "{}",
        render(&swarm, format, started.elapsed().as_secs_f64())
    );
    0
}

#[cfg(test)]
mod tests;
