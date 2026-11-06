//! The model as `panorama snapshot --json` prints it: every field of the model in its order
//! (plan `uniform-agent-tree.md` §2.1). It began as Python's `Swarm.to_json`; since U6 it is
//! the uniform tree, `roots` with one flat `agents` list each and the agents the done window
//! left out as `hidden_finished` (§6: a breaking CLI output change).

use super::pyjson::J;
use panorama_core::model::{Agent, BgTask, Counts, Root, Swarm};

pub(crate) fn strs(items: &[String]) -> J {
    J::Arr(items.iter().map(J::str).collect())
}

pub(crate) fn opt_int(v: Option<i64>) -> J {
    v.map_or(J::Null, J::Int)
}

pub(crate) fn bg(items: &[BgTask]) -> J {
    J::Arr(
        items
            .iter()
            .map(|b| {
                J::obj([
                    ("id", J::str(&b.id)),
                    ("kind", J::str(&b.kind)),
                    ("description", J::str(&b.description)),
                    ("started", J::opt_float(b.started)),
                ])
            })
            .collect(),
    )
}

/// One agent, every field.
pub fn agent(a: &Agent) -> J {
    J::obj([
        ("id", J::str(&a.id)),
        ("title", J::str(&a.title)),
        ("title_source", J::str(&a.title_source)),
        ("description", J::str(&a.description)),
        ("role", J::str(&a.role)),
        ("agent_type", J::opt_str(a.agent_type.clone())),
        ("model", J::opt_str(a.model.clone())),
        ("status", J::str(a.status.code())),
        ("tool_use_id", J::opt_str(a.tool_use_id.clone())),
        ("started", J::opt_float(a.started)),
        ("ended", J::opt_float(a.ended)),
        ("last_at", J::opt_float(a.last_at)),
        ("tool_calls", J::Int(a.tool_calls)),
        ("activity", J::str(&a.activity)),
        ("last_message", J::str(&a.last_message)),
        ("questions", strs(&a.questions)),
        ("pending", strs(&a.pending)),
        ("question_ids", J::ints(a.question_ids.iter().copied())),
        ("error", J::opt_str(a.error.clone())),
        ("parent", J::opt_str(a.parent.clone())),
        ("depth", J::Int(a.depth)),
        ("resumes", J::Int(a.resumes)),
        ("inferred", J::Bool(a.inferred)),
        ("transcript", J::opt_str(a.transcript.clone())),
        ("seq", J::Int(a.seq)),
        ("bg", bg(&a.bg)),
    ])
}

fn root(o: &Root) -> J {
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
        ("transcript", J::opt_str(o.transcript.clone())),
        ("pending", strs(&o.pending)),
        ("inbox", J::ints(o.inbox.iter().copied())),
        ("inbox_texts", strs(&o.inbox_texts)),
        ("agents", J::Arr(o.agents.iter().map(agent).collect())),
        ("hidden_finished", J::Int(o.hidden_finished)),
        ("tab_no", opt_int(o.tab_no)),
        ("machine", J::opt_str(o.machine.clone())),
        ("machine_id", J::opt_str(o.machine_id.clone())),
        ("bg", bg(&o.bg)),
        ("last_activity", J::opt_float(o.last_activity)),
    ])
}

/// `Swarm::counts`.
pub fn counts(c: &Counts) -> Vec<(String, J)> {
    [
        ("roots", c.roots),
        ("agents", c.agents),
        ("running", c.running),
        ("waiting", c.waiting),
        ("questions", c.questions),
        ("blocked", c.blocked),
        ("failed", c.failed),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), J::Int(v as i64)))
    .collect()
}

/// The whole swarm.
pub fn swarm(s: &Swarm) -> J {
    J::obj([
        ("generated_at", J::Float(s.generated_at)),
        ("herdr_ok", J::Bool(s.herdr_ok)),
        ("inbox_ok", J::Bool(s.inbox_ok)),
        ("errors", strs(&s.errors)),
        ("counts", J::Obj(counts(&s.counts()))),
        ("roots", J::Arr(s.roots.iter().map(root).collect())),
    ])
}
