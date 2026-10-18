//! The swarm model from herdr's snapshot and the transcripts (Python `collect.py`): which
//! sessions are live or blocked (herdr), one kit [`Scanner`] per Claude Code session (agents,
//! statuses, nesting), thread grouping, the explicit thread channel, and herdr-inbox's open
//! questions.
//!
//! A [`Collector`] lives as long as the view; each [`Collector::collect`] stats unchanged
//! files and reads only the appended bytes of changed ones.
//!
//! Stated deviations: the kit's fold never fails a record, so there is no "N unreadable
//! records" error; and an agent's `agent_type` or `tool_use_id` that is not a string in the
//! transcript reads as its text (Python passes the JSON value through).
//!
//! [`Scanner`]: herdr_agent_kit::agent_session::claude::Scanner

pub mod agents;
pub mod events;
pub mod liveness;
pub mod mirror;
mod orchestrator;
pub mod paths;
pub mod py;
pub mod sessions;
pub mod text;
pub mod threads;

pub use orchestrator::pending_prompt;

use events::Channel;
use herdr_agent_kit::internal::inbox_contract::sqlite::OpenQuestion;
use liveness::{PanePid, PanePids};
use panorama_core::model::Swarm;
use py::str_of;
use serde_json::{Map, Value};
use sessions::Sessions;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// Where a collector reads, and how.
pub struct Options {
    /// Claude Code's `projects` dir (or a remote machine's mirror of it).
    pub projects: PathBuf,
    /// The explicit thread channel's dir.
    pub events_dir: PathBuf,
    /// The hook records' dir.
    pub hooks_dir: PathBuf,
    /// Whether background tasks can be checked against this machine's processes (false for
    /// another machine's mirrored transcripts).
    pub procs: bool,
    /// Finds a pane's Claude Code process.
    pub pane_pid: Box<PanePid>,
}

/// The collector: its sessions, channels and pane processes, kept across collects.
pub struct Collector {
    sessions: Sessions,
    channels: HashMap<String, Channel>,
    events_dir: PathBuf,
    procs: bool,
    pids: RefCell<PanePids>,
    window: f64,
    /// The view's own pane: the agent in its tab is the one it is docked by.
    pub self_pane: Option<String>,
}

/// The snapshot's tabs by id (a repeated id: the last one, in the first one's place).
fn tabs_of(snap: &Map<String, Value>) -> Vec<(Value, &Map<String, Value>)> {
    let mut tabs: Vec<(Value, &Map<String, Value>)> = Vec::new();
    for t in snap
        .get("tabs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        let tid = t.get("tab_id").cloned().unwrap_or(Value::Null);
        match tabs.iter_mut().find(|(k, _)| *k == tid) {
            Some(slot) => slot.1 = t,
            None => tabs.push((tid, t)),
        }
    }
    tabs
}

fn tab_of(a: &Map<String, Value>) -> Value {
    a.get("tab_id").cloned().unwrap_or(Value::Null)
}

impl Collector {
    /// A collector reading as `options` say.
    pub fn new(options: Options) -> Self {
        Self {
            sessions: Sessions::new(options.projects, options.hooks_dir),
            channels: HashMap::new(),
            events_dir: options.events_dir,
            procs: options.procs,
            pids: RefCell::new(PanePids::new(options.pane_pid)),
            window: 0.0,
            self_pane: None,
        }
    }

    /// The sessions these ones continued in, as far as the transcripts read so far tell.
    pub fn continuations(&self, session_ids: &[String]) -> Vec<String> {
        self.sessions.continuations(session_ids)
    }

    /// The swarm at `now`: herdr's `snapshot` (`None`: herdr is down), the inbox's open
    /// questions (`None`: no inbox), finished threads shown for `window` seconds, idle
    /// orchestrators stale after `stale_after`.
    pub fn collect(
        &mut self,
        snapshot: Option<&Value>,
        open_qs: Option<&[OpenQuestion]>,
        now: f64,
        window: f64,
        stale_after: f64,
    ) -> Swarm {
        self.window = window;
        let mut swarm = Swarm {
            generated_at: now,
            herdr_ok: snapshot.is_some(),
            inbox_ok: open_qs.is_some(),
            stale_after,
            ..Swarm::default()
        };
        let Some(snap) = snapshot
            .and_then(Value::as_object)
            .filter(|s| !s.is_empty())
        else {
            return swarm;
        };
        let tabs = tabs_of(snap);
        let order = |tid: &Value| tabs.iter().position(|(k, _)| k == tid).unwrap_or(999);
        let agents: Vec<&Map<String, Value>> = snap
            .get("agents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter(|a| str_of(a, "agent") == Some("claude"))
            .collect();
        let per_tab =
            |a: &Map<String, Value>| agents.iter().filter(|b| tab_of(b) == tab_of(a)).count();
        let mut sorted = agents.clone();
        sorted.sort_by(|a, b| {
            (order(&tab_of(a)), str_of(a, "pane_id").unwrap_or(""))
                .cmp(&(order(&tab_of(b)), str_of(b, "pane_id").unwrap_or("")))
        });
        let tab_no = |tid: &Value| -> Option<i64> {
            let (_, t) = tabs.iter().find(|(k, _)| k == tid)?;
            let ws = t.get("workspace_id");
            let pos = tabs
                .iter()
                .filter(|(_, x)| x.get("workspace_id") == ws)
                .position(|(k, _)| k == tid)?;
            Some(pos as i64 + 1)
        };
        swarm.docked = self.docked(snap, &sorted);
        let empty = Map::new();
        for a in sorted {
            let tab = tabs
                .iter()
                .find(|(k, _)| *k == tab_of(a))
                .map_or(&empty, |(_, t)| *t);
            let mut o = self.orchestrator(a, tab, per_tab(a), open_qs.unwrap_or(&[]), now);
            o.tab_no = tab_no(&tab_of(a));
            swarm.orchestrators.push(o);
        }
        swarm
    }

    /// The agent in the view's own tab, if the view runs in herdr.
    fn docked(&self, snap: &Map<String, Value>, agents: &[&Map<String, Value>]) -> Option<String> {
        let me = self.self_pane.as_deref()?;
        let pane = snap
            .get("panes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .find(|p| str_of(p, "pane_id") == Some(me))?;
        let tab = pane.get("tab_id");
        agents
            .iter()
            .find(|a| a.get("tab_id") == tab)
            .and_then(|a| str_of(a, "pane_id"))
            .map(str::to_owned)
    }
}
