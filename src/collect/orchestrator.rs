//! One orchestrator from one herdr agent (Python `Collector._orchestrator`): its ids and
//! herdr's word on it, the session it lives in now, its inbox questions, its label, and,
//! when its transcript is found, its background work, pending prompt and threads.

use super::Collector;
use super::agents::{Ctx, bg_tasks};
use super::events::Channel;
use super::liveness::PaneLiveness;
use super::paths::events_path;
use super::py::{py_str, str_of, truthy, truthy_str};
use super::sessions::since;
use super::text::strip_inbox_mark;
use super::threads::{finish::flavour, main_card::main_session_thread, threads_of};
use super::title;
use herdr_agent_kit::agent_procs::{Liveness, NoProcs};
use herdr_agent_kit::agent_session::Ts;
use herdr_agent_kit::agent_session::claude::wire::{self, ToolCall};
use herdr_agent_kit::agent_session::claude::{Owner, Session};
use herdr_agent_kit::internal::inbox_contract::sqlite::OpenQuestion;
use panorama_core::model::{HerdrStatus, Orchestrator};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// What a blocked session waits on: its AskUserQuestion texts, or `wants Tool(...)`.
pub fn pending_prompt(pending: &[ToolCall]) -> Vec<String> {
    let Some(last) = pending.last() else {
        return Vec::new();
    };
    if last.name == "AskUserQuestion" {
        return last
            .input
            .get("questions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter_map(|q| truthy_str(q, "question"))
            .collect();
    }
    vec![format!(
        "wants {}",
        wire::summarize_tool(&last.name, Some(&last.input), None)
    )]
}

/// A JSON value as an optional id (Python passes herdr's value through).
fn opt_id(v: Option<&Value>) -> Option<String> {
    v.filter(|v| !v.is_null()).map(py_str)
}

/// The orchestrator with no session read yet.
fn bare(a: &Map<String, Value>, sid: Option<&Value>) -> Orchestrator {
    Orchestrator {
        harness: str_of(a, "agent").unwrap_or("claude").to_owned(),
        pane_id: truthy_str(a, "pane_id").unwrap_or_default(),
        tab_id: opt_id(a.get("tab_id")),
        tab_label: String::new(),
        title: String::new(),
        title_source: String::new(),
        session_id: opt_id(sid),
        herdr_status: HerdrStatus::from_code(
            &truthy_str(a, "agent_status").unwrap_or_else(|| "unknown".into()),
        ),
        workspace_id: opt_id(a.get("workspace_id")),
        focused: a.get("focused").is_some_and(truthy),
        cwd: str_of(a, "cwd").map(str::to_owned),
        flavour: "agents".into(),
        transcript: None,
        pending: Vec::new(),
        inbox: Vec::new(),
        inbox_texts: Vec::new(),
        threads: Vec::new(),
        hidden_done: 0,
        tab_no: None,
        machine: None,
        machine_id: None,
        bg: Vec::new(),
        last_activity: None,
    }
}

/// The label: the tab's (without the inbox mark), else the session's titles, else herdr's
/// terminal title, else the pane; with the pane id when the tab holds several agents.
fn label(
    a: &Map<String, Value>,
    tab: &Map<String, Value>,
    sess: Option<&Session>,
    pane: &str,
    per_tab: usize,
) -> String {
    let titles = sess
        .map(|s| s.conversation().titles().clone())
        .unwrap_or_default();
    let own = strip_inbox_mark(str_of(tab, "label").unwrap_or("")).to_owned();
    let label = [Some(own), titles.custom, titles.agent, titles.ai]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .or_else(|| truthy_str(a, "terminal_title_stripped"))
        .unwrap_or_else(|| {
            if pane.is_empty() {
                "?".into()
            } else {
                pane.to_owned()
            }
        });
    if per_tab > 1 {
        format!("{label} · {pane}")
    } else {
        label
    }
}

/// A root's title from its herdr names, its harness's session titles (Claude's `custom-title`,
/// `agent-name`, `ai-title`; a Codex thread's name) and its first prompt (§2.2); `None` leaves
/// the last rung to the caller, which knows the tab's number.
fn root_title(
    a: &Map<String, Value>,
    tab: &Map<String, Value>,
    per_tab: usize,
    session: [Option<&str>; 3],
    first_prompt: Option<&str>,
) -> Option<(String, &'static str)> {
    title::root(&title::RootInputs {
        kind: str_of(a, "agent").unwrap_or("claude"),
        pane_name: str_of(a, "name"),
        tab_label: strip_inbox_mark(str_of(tab, "label").unwrap_or("")),
        per_tab,
        session,
        first_prompt,
        terminal_title: str_of(a, "terminal_title_stripped"),
    })
}

impl Collector {
    /// The orchestrator of herdr agent `a` in `tab`.
    pub(super) fn orchestrator(
        &mut self,
        a: &Map<String, Value>,
        tab: &Map<String, Value>,
        per_tab: usize,
        open_qs: &[OpenQuestion],
        now: f64,
    ) -> Orchestrator {
        let empty = Map::new();
        let info = a
            .get("agent_session")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let sid = if info.get("kind").is_none_or(|k| k == "id") {
            info.get("value")
        } else {
            None
        };
        let mut o = bare(a, sid);
        // Codex has its own history and identity rules. In particular, never look up a
        // Codex id (or cwd) in Claude's continuation or transcript index.
        if o.harness == "codex" {
            o.tab_label = label(a, tab, None, &o.pane_id, per_tab);
            let native = super::codex::native_id(info);
            o.session_id = if self.core_source {
                (info.get("source").and_then(Value::as_str) == Some("panorama:core"))
                    .then(|| info.get("value").and_then(Value::as_str))
                    .flatten()
                    .filter(|id| crate::codex::identity::valid_thread_id(id))
                    .map(str::to_owned)
            } else {
                match self.codex_identity.as_mut() {
                    Some(identity) => identity.resolve(&o.pane_id, native),
                    None => native.map(str::to_owned),
                }
            };
            let seen = self
                .codex
                .as_mut()
                .map(|details| details.enrich(&mut o, now, self.window))
                .unwrap_or_default();
            let session = [seen.name.as_deref(), None, None];
            if let Some((t, src)) = root_title(a, tab, per_tab, session, seen.prompt.as_deref()) {
                (o.title, o.title_source) = (t, src.to_owned());
            }
            return o;
        }
        let since = since(now, self.window);
        let cwd = str_of(a, "cwd");
        let (ids, key) = match sid.filter(|v| truthy(v)) {
            Some(Value::String(s)) => self.sessions.live_session(s, cwd, since),
            Some(other) => (vec![py_str(other)], None),
            None => (Vec::new(), None),
        };
        if let Some(k) = &key {
            o.session_id = Some(k.clone());
        }
        let mine: Vec<&OpenQuestion> = open_qs
            .iter()
            .filter(
                |q| match q.session_id.as_deref().filter(|s| !s.is_empty()) {
                    Some(s) => ids.iter().any(|i| i == s),
                    None => q.pane_id.as_deref() == Some(o.pane_id.as_str()),
                },
            )
            .collect();
        o.inbox = mine.iter().map(|q| q.id).collect();
        o.inbox_texts = mine.iter().map(|q| q.question.clone()).collect();
        let scanner = key.as_ref().and_then(|k| self.sessions.get(k));
        o.tab_label = label(a, tab, scanner.map(|s| s.session()), &o.pane_id, per_tab);
        let conv = scanner.map(|s| s.session().conversation());
        let titles = conv.map(|c| c.titles());
        let session = titles.map_or([None; 3], |t| {
            [t.custom.as_deref(), t.agent.as_deref(), t.ai.as_deref()]
        });
        let prompt = conv.and_then(|c| c.prompts().next().map(|p| p.text));
        if let Some((t, src)) = root_title(a, tab, per_tab, session, prompt) {
            (o.title, o.title_source) = (t, src.to_owned());
        }
        let (Some(key), Some(scanner)) = (key.clone(), scanner) else {
            return o;
        };
        o.transcript = Some(scanner.root().to_string_lossy().into_owned());
        let channel = self
            .channels
            .entry(key.clone())
            .or_insert_with(|| Channel::new(events_path(&self.events_dir, &key)));
        channel.update();
        let qtext: HashMap<i64, String> = mine.iter().map(|q| (q.id, q.question.clone())).collect();
        let pane = o.pane_id.clone();
        let pane_live = PaneLiveness::new(&pane, now, &self.pids);
        let live: &dyn Liveness = if self.procs { &pane_live } else { &NoProcs };
        let sess = self.sessions.get(&key).expect("found above").session();
        let waiting: Vec<_> = sess
            .background()
            .waiting(&Owner::Main, Ts(now), live)
            .collect();
        o.bg = bg_tasks(sess, &waiting);
        o.last_activity = sess.last_activity().map(Ts::secs);
        if o.herdr_status == HerdrStatus::Blocked
            && let Some(main) = sess.file(&Owner::Main)
        {
            o.pending = pending_prompt(&main.info.pending);
        }
        let cx = Ctx {
            sess,
            qtext: &qtext,
            now,
            window: self.window,
            live,
        };
        let (drafts, mut arena, hidden) = threads_of(&cx, &self.channels[&key]);
        o.hidden_done = hidden;
        o.flavour = flavour(&drafts);
        let card = main_session_thread(&o, &drafts, &arena, sess);
        let mut threads: Vec<_> = drafts.into_iter().map(|d| d.into_thread(&arena)).collect();
        if let Some((d, ag)) = card {
            arena.insert(ag.id.clone(), ag);
            threads.insert(0, d.into_thread(&arena));
        }
        o.threads = threads;
        o
    }
}
