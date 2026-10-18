//! The model as `panorama snapshot --json` prints it (Python `Swarm.to_json`, which is
//! `dataclasses.asdict`): every field in the dataclass's order.

use super::pyjson::J;
use panorama_core::model::{Agent, BgTask, Counts, Orchestrator, Phase, Swarm, Thread};

fn strs(items: &[String]) -> J {
    J::Arr(items.iter().map(J::str).collect())
}

fn opt_int(v: Option<i64>) -> J {
    v.map_or(J::Null, J::Int)
}

fn bg(items: &[BgTask]) -> J {
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

fn agent(a: &Agent) -> J {
    J::obj([
        ("id", J::str(&a.id)),
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
        ("phases", J::ints(a.phases.iter().copied())),
        ("parent", J::opt_str(a.parent.clone())),
        ("depth", J::Int(a.depth)),
        ("resumes", J::Int(a.resumes)),
        ("inferred", J::Bool(a.inferred)),
        ("transcript", J::opt_str(a.transcript.clone())),
        ("seq", J::Int(a.seq)),
        ("bg", bg(&a.bg)),
    ])
}

fn phase(p: &Phase) -> J {
    J::obj([
        ("name", J::str(&p.name)),
        ("state", J::str(p.state.code())),
        ("start", J::opt_float(p.start)),
        ("end", J::opt_float(p.end)),
        ("note", J::opt_str(p.note.clone())),
    ])
}

fn thread(t: &Thread) -> J {
    J::obj([
        ("id", J::str(&t.id)),
        ("name", J::str(&t.name)),
        ("source", J::str(t.source.code())),
        ("phases", J::Arr(t.phases.iter().map(phase).collect())),
        ("agents", J::Arr(t.agents.iter().map(agent).collect())),
        ("status", J::str(t.status.code())),
        ("started", J::opt_float(t.started)),
        ("ended", J::opt_float(t.ended)),
        ("worktree", J::opt_str(t.worktree.clone())),
        ("pct", opt_int(t.pct)),
        ("flavour", J::opt_str(t.flavour.clone())),
    ])
}

fn orchestrator(o: &Orchestrator) -> J {
    J::obj([
        ("pane_id", J::str(&o.pane_id)),
        ("tab_id", J::opt_str(o.tab_id.clone())),
        ("tab_label", J::str(&o.tab_label)),
        ("session_id", J::opt_str(o.session_id.clone())),
        ("herdr_status", J::str(o.herdr_status.code())),
        ("workspace_id", J::opt_str(o.workspace_id.clone())),
        ("focused", J::Bool(o.focused)),
        ("cwd", J::opt_str(o.cwd.clone())),
        ("flavour", J::str(&o.flavour)),
        ("transcript", J::opt_str(o.transcript.clone())),
        ("pending", strs(&o.pending)),
        ("inbox", J::ints(o.inbox.iter().copied())),
        ("inbox_texts", strs(&o.inbox_texts)),
        ("threads", J::Arr(o.threads.iter().map(thread).collect())),
        ("hidden_done", J::Int(o.hidden_done)),
        ("tab_no", opt_int(o.tab_no)),
        ("machine", J::opt_str(o.machine.clone())),
        ("machine_id", J::opt_str(o.machine_id.clone())),
        ("bg", bg(&o.bg)),
        ("last_activity", J::opt_float(o.last_activity)),
    ])
}

/// `Swarm.counts()`.
pub fn counts(c: &Counts) -> Vec<(String, J)> {
    [
        ("orchestrators", c.orchestrators),
        ("threads", c.threads),
        ("active_threads", c.active_threads),
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

/// `Swarm.to_json()`.
pub fn swarm(s: &Swarm) -> J {
    J::obj([
        ("generated_at", J::Float(s.generated_at)),
        ("herdr_ok", J::Bool(s.herdr_ok)),
        ("inbox_ok", J::Bool(s.inbox_ok)),
        ("errors", strs(&s.errors)),
        ("counts", J::Obj(counts(&s.counts()))),
        (
            "orchestrators",
            J::Arr(s.orchestrators.iter().map(orchestrator).collect()),
        ),
    ])
}
