//! The swarm model from herdr's snapshot and the transcripts (Python `collect.py`): which
//! sessions are live or blocked (herdr), one kit [`Scanner`] per Claude Code session (agents,
//! statuses, nesting) built into one agent tree per root ([`tree`]), and herdr-inbox's open
//! questions. The explicit thread channel's files are no longer read (plan
//! `uniform-agent-tree.md` §6).
//!
//! A [`Collector`] lives as long as the view; each [`Collector::collect`] stats unchanged
//! files and reads only the appended bytes of changed ones.
//!
//! Stated deviations:
//! - the kit's fold never fails a record, so there is no "N unreadable records" error;
//! - an agent's `agent_type` or `tool_use_id`, or herdr's `agent_session.value`, that is not a
//!   string reads as its text (Python passes the JSON value through);
//! - where Python raises inside one orchestrator and reports it in `errors` (a list as an
//!   agent's `agentType`: unhashable; a non-string `cwd`), Rust shows the orchestrator: those
//!   values are read as text or as absent. A panic is still contained per orchestrator
//!   ([`contain`]).
//!
//! [`Scanner`]: herdr_agent_kit::agent_session::claude::Scanner

pub mod agents;
mod codex;
pub mod liveness;
pub mod mirror;
mod orchestrator;
pub mod paths;
pub mod py;
pub mod roles;
pub mod sessions;
pub mod text;
pub mod title;
pub mod tree;

pub use orchestrator::pending_prompt;

use herdr_agent_kit::internal::inbox_contract::sqlite::OpenQuestion;
use liveness::{PanePid, PanePids};
use panorama_core::model::Swarm;
use py::str_of;
use serde_json::{Map, Value};
use sessions::Sessions;
use std::cell::RefCell;
use std::path::PathBuf;

/// Where a collector reads, and how.
pub struct Options {
    /// Claude Code's `projects` dir (or a remote machine's mirror of it).
    pub projects: PathBuf,
    /// The hook records' dir.
    pub hooks_dir: PathBuf,
    /// Whether background tasks can be checked against this machine's processes (false for
    /// another machine's mirrored transcripts).
    pub procs: bool,
    /// Finds a pane's Claude Code process.
    pub pane_pid: Box<PanePid>,
}

/// The collector: its sessions and pane processes, kept across collects.
pub struct Collector {
    codex: Option<codex::Details>,
    codex_identity: Option<codex::identity::LocalIdentity>,
    sessions: Sessions,
    procs: bool,
    pids: RefCell<PanePids>,
    window: f64,
    core_source: bool,
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

/// One orchestrator's collect, contained: a panic in it (untrusted transcript text) becomes
/// an entry in `errors`, `<pane>: panic: <message>` (at most 300 characters), and the other
/// orchestrators are still collected (Python's `try/except` around `_orchestrator`).
pub fn contain<T>(pane: &str, f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| {
        let msg = p
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        py::chars(&format!("{pane}: panic: {msg}"), 300)
    })
}

impl Collector {
    /// A collector reading as `options` say.
    pub fn new(options: Options) -> Self {
        Self {
            codex: None,
            codex_identity: None,
            sessions: Sessions::new(options.projects, options.hooks_dir),
            procs: options.procs,
            pids: RefCell::new(PanePids::new(options.pane_pid)),
            window: 0.0,
            core_source: false,
            self_pane: None,
        }
    }

    /// Only the core adapter supplies sessions after exact process-backed ownership proof.
    pub fn with_core_source(mut self) -> Self {
        self.core_source = true;
        self
    }

    /// Enable local Codex observations explicitly. Remote mirrors and bare test
    /// collectors never discover or read this machine's Codex home implicitly.
    pub fn with_codex(mut self, options: crate::codex::Options) -> Self {
        self.codex = Some(codex::Details::new(options));
        self
    }

    /// Resolve local plugin associations against the current pane process.
    /// Mirrors do not install this resolver and cannot read local bindings.
    pub fn with_codex_identity(
        mut self,
        env: herdr_agent_kit::plugin_env::PluginEnv,
        herdr: herdr_agent_kit::herdr_api::blocking::Herdr,
    ) -> Self {
        self.codex_identity = Some(codex::identity::LocalIdentity::new(env, herdr));
        self
    }

    /// Whether the current snapshot has queued local history reads. Only the
    /// one-shot CLI uses this to warm up; interactive collection never waits.
    pub fn codex_pending(&self, swarm: &Swarm) -> bool {
        self.codex.as_ref().is_some_and(|details| {
            swarm
                .roots
                .iter()
                .any(|orchestrator| details.pending(orchestrator))
        })
    }

    /// The sessions these ones continued in, as far as the transcripts read so far tell.
    pub fn continuations(&self, session_ids: &[String]) -> Vec<String> {
        self.sessions.continuations(session_ids)
    }

    /// The swarm at `now`: herdr's `snapshot` (`None`: herdr is down), the inbox's open
    /// questions (`None`: no inbox), finished agents shown for `window` seconds, idle
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
        if self.core_source {
            self.pids.borrow_mut().clear();
        }
        if let Some(identity) = self.codex_identity.as_mut() {
            identity.begin_collect();
        }
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
            .filter(|a| matches!(str_of(a, "agent"), Some("claude" | "codex")))
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
            let n = per_tab(a);
            let pane = a
                .get("pane_id")
                .map_or_else(|| "None".to_owned(), py::py_str);
            match contain(&pane, || {
                self.orchestrator(a, tab, n, open_qs.unwrap_or(&[]), now)
            }) {
                Ok((mut o, agents)) => {
                    o.tab_no = tab_no(&tab_of(a));
                    if o.title.is_empty() {
                        let (t, src) = title::root_fallback(&o.harness, o.tab_no, &o.pane_id);
                        (o.title, o.title_source) = (t, src.to_owned());
                    }
                    tree::plant(&mut o, agents);
                    swarm.roots.push(o);
                }
                Err(e) => swarm.errors.push(e),
            }
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

#[cfg(test)]
mod tests;
