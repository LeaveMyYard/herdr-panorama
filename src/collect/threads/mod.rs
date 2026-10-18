//! A session's threads (Python `Collector._threads`, `threads.py`). Sources, strongest
//! first; an agent belongs to the first that claims it: the explicit channel
//! ([`declared`]), a native Workflow run ([`workflow`]), a PR or issue reference, the operator
//! turn ([`heuristic`]). A nested agent joins its root agent's thread (and its parent's last
//! phase); a root no thread holds gets an orphan thread. Finished threads older than the done
//! window are hidden and counted.

pub mod declared;
pub mod finish;
pub mod heuristic;
pub mod main_card;
pub mod phases;
pub mod roles;
pub mod spans;
pub mod workflow;

use super::agents::{Arena, Ctx, agents_of};
use super::events::Channel;
use herdr_agent_kit::agent_session::Ts;
use panorama_core::model::{Thread, ThreadSource, ThreadStatus};
use std::collections::{HashMap, HashSet};

/// A thread being built: the thread, and its agents as ids into the arena.
#[derive(Clone, Debug)]
pub struct Draft {
    pub thread: Thread,
    pub ids: Vec<String>,
}

impl Draft {
    /// A thread with no phases, no agents and no status yet.
    pub fn new(id: String, name: String, source: ThreadSource) -> Self {
        let thread = Thread {
            id,
            name,
            source,
            phases: Vec::new(),
            agents: Vec::new(),
            status: ThreadStatus::from_code(""),
            started: None,
            ended: None,
            worktree: None,
            pct: None,
            flavour: None,
        };
        Self {
            thread,
            ids: Vec::new(),
        }
    }

    /// The thread with its agents.
    pub fn into_thread(self, arena: &Arena) -> Thread {
        let mut t = self.thread;
        t.agents = self.ids.iter().map(|id| arena[id].clone()).collect();
        t
    }
}

/// `id` and the agents that spawned it, up to its top one (loop-safe).
fn chain(id: &str, arena: &Arena) -> Vec<String> {
    let mut c = vec![id.to_owned()];
    while let Some(p) = arena[c.last().expect("non-empty").as_str()].parent.clone() {
        if !arena.contains_key(&p) || c.contains(&p) {
            break;
        }
        c.push(p);
    }
    c
}

/// Every agent no thread holds joins its root's thread (an orphan thread when the root has
/// none), parents first, taking its parent's last phase.
fn place_the_rest(out: &mut Vec<Draft>, arena: &mut Arena) {
    let mut where_: HashMap<String, usize> = HashMap::new();
    for (i, d) in out.iter().enumerate() {
        for a in &d.ids {
            where_.insert(a.clone(), i);
        }
    }
    let mut rest: Vec<(usize, i64, String)> = arena
        .values()
        .map(|a| (chain(&a.id, arena).len(), a.seq, a.id.clone()))
        .collect();
    rest.sort_by_key(|x| (x.0, x.1));
    for (_, _, id) in rest {
        if where_.contains_key(&id) {
            continue;
        }
        let root = chain(&id, arena).pop().expect("non-empty");
        let ti = match where_.get(&root) {
            Some(&ti) => ti,
            None => {
                let r = &arena[&root];
                let name = if r.description.is_empty() {
                    r.id.clone()
                } else {
                    r.description.clone()
                };
                out.push(Draft::new(
                    format!("orphan:{root}"),
                    name,
                    ThreadSource::Turn,
                ));
                where_.insert(root.clone(), out.len() - 1);
                out.len() - 1
            }
        };
        out[ti].ids.push(id.clone());
        let parent_phase = arena[&id]
            .parent
            .as_ref()
            .and_then(|p| arena.get(p))
            .and_then(|p| p.phases.last().copied());
        if let Some(ph) = parent_phase {
            arena[&id].phases = vec![ph];
        }
        where_.insert(id, ti);
    }
}

/// The threads shown (sorted), the agents, and how many finished threads were hidden.
pub fn threads_of(cx: &Ctx, channel: &Channel) -> (Vec<Draft>, Arena, i64) {
    let sess = cx.sess;
    let mut arena = agents_of(cx);
    let mut claimed: HashSet<String> = HashSet::new();
    let mut out: Vec<Draft> = channel
        .threads
        .iter()
        .map(|d| declared::declared_thread(d, &mut arena, &mut claimed, cx.now))
        .collect();
    let mut runs: Vec<String> = sess
        .agents()
        .filter_map(|(_, r)| r.workflow.clone())
        .chain(sess.workflows().launches().map(|w| w.run_id.clone()))
        .collect();
    runs.sort();
    runs.dedup();
    out.extend(
        runs.iter()
            .filter_map(|run| workflow::workflow_thread(sess, run, &mut arena, &mut claimed)),
    );
    let top: Vec<String> = arena
        .values()
        .filter(|a| !claimed.contains(&a.id) && a.parent.is_none())
        .map(|a| a.id.clone())
        .collect();
    let mut turn_of: HashMap<String, i64> = HashMap::new();
    let mut spans = HashMap::new();
    for id in &top {
        let a = &arena[id];
        if let Some(p) = sess.conversation().prompt_before(a.started.map(Ts)) {
            turn_of.insert(id.clone(), p.index as i64);
        }
        spans.insert(id.clone(), spans::agent_spans(sess, a));
    }
    out.extend(heuristic::group_refs_and_turns(
        &top, &turn_of, &spans, &mut arena,
    ));
    place_the_rest(&mut out, &mut arena);
    let mut kept = Vec::new();
    let mut hidden = 0;
    for mut d in out {
        finish::finish(&mut d, &arena);
        let t = &d.thread;
        if (t.is_done() || t.status == ThreadStatus::Failed)
            && t.ended.unwrap_or(0.0) < cx.now - cx.window
        {
            hidden += 1;
            continue;
        }
        kept.push(d);
    }
    (finish::sort_threads(kept, &arena), arena, hidden)
}
