//! The swarm in app feed v1's legacy shape (`docs/protocol/app-feed-v1.md`), which the app
//! mods read until feed v2 (plan `uniform-agent-tree.md` U7): `orchestrators`, each with
//! exactly one thread (`agents`, source `session`, no phases) holding its whole agent tree,
//! flavour `agents`, no hidden done threads, and the 1.x counts.
//!
//! This is what the U3 collectors' single-thread adapter put on every wire, now built here
//! from the uniform tree, so a v1 feed reads byte for byte as before.

use crate::collect::py::{or_time, truthy_time};
use crate::snapshot::{json, pyjson::J};
use panorama_core::model::{Agent, Root, Status, Swarm};

/// The id of the one thread every root carries in feed v1.
pub const TREE_THREAD: &str = "agents";

/// The thread's status (Python `threads.thread_status`): active while one agent is live and
/// not failed; else failed when the newest failed, cancelled when all were, done otherwise
/// (and done when there are none).
pub fn status(agents: &[Agent]) -> &'static str {
    if agents
        .iter()
        .any(|a| a.status.is_live() && a.status != Status::Failed)
    {
        return "active";
    }
    let key = |a: &Agent| or_time(a.ended, a.last_at).unwrap_or(0.0);
    let newest = agents.iter().fold(None::<&Agent>, |best, a| match best {
        Some(b) if key(b) >= key(a) => Some(b),
        _ => Some(a),
    });
    match newest {
        None => "done",
        Some(n) if n.status == Status::Failed => "failed",
        Some(_) if agents.iter().all(|a| a.status == Status::Cancelled) => "cancelled",
        Some(_) => "done",
    }
}

fn agent(a: &Agent) -> J {
    let J::Obj(mut fields) = json::agent(a) else {
        unreachable!("an agent is an object")
    };
    fields.push(("phases".into(), J::Arr(Vec::new())));
    J::Obj(fields)
}

fn thread(o: &Root) -> J {
    let status = status(&o.agents);
    let started = o
        .agents
        .iter()
        .filter_map(|a| truthy_time(a.started))
        .reduce(f64::min);
    let ended = (status != "active")
        .then(|| {
            let last = o
                .agents
                .iter()
                .filter_map(|a| or_time(a.ended, a.last_at))
                .filter(|e| *e != 0.0)
                .reduce(f64::max);
            last.or(started)
        })
        .flatten();
    J::obj([
        ("id", J::str(TREE_THREAD)),
        ("name", J::str(o.name())),
        ("source", J::str("session")),
        ("phases", J::Arr(Vec::new())),
        ("agents", J::Arr(o.agents.iter().map(agent).collect())),
        ("status", J::str(status)),
        ("started", J::opt_float(started)),
        ("ended", J::opt_float(ended)),
        ("worktree", J::Null),
        ("pct", J::Null),
        ("flavour", J::Null),
    ])
}

fn orchestrator(o: &Root) -> J {
    J::obj([
        ("pane_id", J::str(&o.pane_id)),
        ("harness", J::str(&o.harness)),
        ("tab_id", J::opt_str(o.tab_id.clone())),
        ("tab_label", J::str(&o.tab_label)),
        ("title", J::str(&o.title)),
        ("title_source", J::str(&o.title_source)),
        ("session_id", J::opt_str(o.session_id.clone())),
        ("herdr_status", J::str(o.herdr_status.code())),
        ("workspace_id", J::opt_str(o.workspace_id.clone())),
        ("focused", J::Bool(o.focused)),
        ("cwd", J::opt_str(o.cwd.clone())),
        ("flavour", J::str("agents")),
        ("transcript", J::opt_str(o.transcript.clone())),
        ("pending", json::strs(&o.pending)),
        ("inbox", J::ints(o.inbox.iter().copied())),
        ("inbox_texts", json::strs(&o.inbox_texts)),
        ("threads", J::Arr(vec![thread(o)])),
        ("hidden_done", J::Int(0)),
        ("tab_no", json::opt_int(o.tab_no)),
        ("machine", J::opt_str(o.machine.clone())),
        ("machine_id", J::opt_str(o.machine_id.clone())),
        ("bg", json::bg(&o.bg)),
        ("last_activity", J::opt_float(o.last_activity)),
    ])
}

/// The 1.x counts: one thread per root, active unless done or cancelled.
fn counts(s: &Swarm) -> J {
    let c = s.counts();
    let active = s
        .roots
        .iter()
        .filter(|o| !matches!(status(&o.agents), "done" | "cancelled"))
        .count();
    let n = |v: usize| J::Int(v as i64);
    J::obj([
        ("orchestrators", n(c.roots)),
        ("threads", n(c.roots)),
        ("active_threads", n(active)),
        ("agents", n(c.agents)),
        ("running", n(c.running)),
        ("waiting", n(c.waiting)),
        ("questions", n(c.questions)),
        ("blocked", n(c.blocked)),
        ("failed", n(c.failed)),
    ])
}

/// The swarm in feed v1's shape.
pub fn swarm(s: &Swarm) -> J {
    J::obj([
        ("generated_at", J::Float(s.generated_at)),
        ("herdr_ok", J::Bool(s.herdr_ok)),
        ("inbox_ok", J::Bool(s.inbox_ok)),
        ("errors", json::strs(&s.errors)),
        ("counts", counts(s)),
        (
            "orchestrators",
            J::Arr(s.roots.iter().map(orchestrator).collect()),
        ),
    ])
}

#[cfg(test)]
mod tests;
