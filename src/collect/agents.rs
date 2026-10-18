//! A session's agents as the views show them (Python `Collector._agents`, `_agent`): each
//! agent's status (an inferred done reads `inf`, never a plain done), its model, counts, last
//! words, questions and role; an agent that finished outside the done window is left out,
//! unless an agent it spawned is shown (subagents nest at any depth).

use super::py::{chars, or_time, py_int, py_splitlines, py_str, str_of, truthy, truthy_time};
use super::text::{paragraph, short_model};
use super::threads::roles::role_of;
use herdr_agent_kit::agent_procs::Liveness;
use herdr_agent_kit::agent_session::claude::fold::BgLaunch;
use herdr_agent_kit::agent_session::claude::{Owner, Session};
use herdr_agent_kit::agent_session::{Status as Kit, Ts};
use indexmap::IndexMap;
use panorama_core::model::{Agent, BgTask, Status};
use panorama_core::view::text::dur;
use std::collections::{HashMap, HashSet};

/// A running agent silent this long reads "quiet 42m".
pub const QUIET_AFTER: f64 = 30.0 * 60.0;

/// The agents shown, by id, in their stable order.
pub type Arena = IndexMap<String, Agent>;

/// What building a session's agents reads.
pub struct Ctx<'a> {
    pub sess: &'a Session,
    /// Open inbox questions of this session, by id.
    pub qtext: &'a HashMap<i64, String>,
    pub now: f64,
    pub window: f64,
    pub live: &'a dyn Liveness,
}

/// Background work as the model lists it.
pub fn bg_tasks(sess: &Session, launches: &[&BgLaunch]) -> Vec<BgTask> {
    launches
        .iter()
        .map(|l| BgTask {
            id: l.task.clone(),
            kind: l.kind.name().to_owned(),
            description: sess.background().description(l).to_owned(),
            started: l.ts.map(Ts::secs),
        })
        .collect()
}

fn status_of(cx: &Ctx, aid: &str) -> (Status, Option<f64>, bool) {
    let st = cx.sess.status_of(aid, Ts(cx.now), cx.live);
    let status = match st.status {
        Kit::Done if st.inferred => Status::Inferred,
        other => Status::from_code(other.code()),
    };
    (status, st.ended.map(Ts::secs), st.inferred)
}

/// One agent; `None` when it finished outside the done window and `keep_old` is false.
fn build(cx: &Ctx, seq: usize, aid: &str, keep_old: bool) -> Option<Agent> {
    let sess = cx.sess;
    let rec = sess.agent(aid)?;
    let f = sess.file_of(aid);
    let (status, ended, inferred) = status_of(cx, aid);
    let last = sess.last_ts(aid).map(Ts::secs);
    if !keep_old && !status.is_live() && or_time(ended, last).unwrap_or(0.0) < cx.now - cx.window {
        return None;
    }
    let (sp, ack) = (sess.parentage().spawn_of(aid), sess.parentage().ack(aid));
    let meta = &rec.meta;
    let mut ag = Agent {
        id: aid.to_owned(),
        ..Agent::default()
    };
    ag.description = match meta.get("description") {
        Some(v) if truthy(v) => py_str(v),
        _ => sp.map(|s| s.description.clone()).unwrap_or_default(),
    };
    ag.agent_type = match meta.get("agentType") {
        Some(v) if truthy(v) => Some(py_str(v)),
        _ => sp.and_then(|s| s.subagent_type.clone()),
    };
    let model = [
        str_of(meta, "model"),
        ack.and_then(|a| a.model.as_deref()),
        f.and_then(|f| f.info.model.as_deref()),
        sp.and_then(|s| s.model.as_deref()),
    ]
    .into_iter()
    .flatten()
    .find(|m| !m.is_empty());
    ag.model = short_model(model);
    ag.status = status.clone();
    ag.tool_use_id = match meta.get("toolUseId") {
        Some(v) if truthy(v) => Some(py_str(v)),
        _ => sp.map(|s| s.call.clone()),
    };
    ag.started = or_time(
        sp.and_then(|s| s.ts).map(Ts::secs),
        f.and_then(|f| f.info.first_ts).map(|t| t.secs()),
    );
    ag.ended = if status.is_live() { None } else { ended };
    ag.last_at = last;
    ag.inferred = inferred;
    ag.transcript = Some(rec.transcript.to_string_lossy().into_owned());
    ag.seq = seq as i64;
    ag.resumes = sess.evidence().resumes(aid).len() as i64;
    ag.depth = py_int(meta.get("spawnDepth"), 1);
    // the count of the latest notice that has one (Python: max over (time, status, count))
    let counted = sess
        .evidence()
        .notices(aid)
        .filter(|n| n.tool_uses.is_some())
        .max_by(|a, b| {
            a.at.0
                .total_cmp(&b.at.0)
                .then(a.ending.cmp(&b.ending))
                .then(a.tool_uses.cmp(&b.tool_uses))
        })
        .and_then(|n| n.tool_uses);
    ag.tool_calls = [
        f.map_or(0, |f| f.tool_calls as i64),
        counted.map_or(0, |c| c as i64),
        ack.and_then(|a| a.tool_count).unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    ag.parent = match sess.parentage().parent_of(aid) {
        Some(Owner::Agent(p)) => Some(p),
        _ => None,
    };
    if let Some(f) = f {
        ag.activity = f.info.activity.clone();
        ag.last_message = paragraph(&f.info.last_text);
    }
    finish(cx, &mut ag, rec.workflow.as_deref(), last);
    Some(ag)
}

/// Questions, error, background work, role and the quiet note.
fn finish(cx: &Ctx, ag: &mut Agent, run: Option<&str>, last: Option<f64>) {
    let sess = cx.sess;
    ag.question_ids = sess
        .qids(&Owner::Agent(ag.id.clone()))
        .into_iter()
        .flatten()
        .filter_map(|q| i64::try_from(*q).ok())
        .filter(|q| cx.qtext.contains_key(q))
        .collect();
    ag.questions = ag
        .question_ids
        .iter()
        .map(|q| cx.qtext[q].clone())
        .collect();
    if ag.status == Status::Failed {
        let msg = if ag.last_message.is_empty() {
            "failed"
        } else {
            &ag.last_message
        };
        ag.error = Some(chars(
            py_splitlines(msg).first().copied().unwrap_or(""),
            200,
        ));
    }
    if ag.status == Status::Background {
        let waiting: Vec<_> = sess
            .background()
            .waiting(&Owner::Agent(ag.id.clone()), Ts(cx.now), cx.live)
            .collect();
        ag.bg = bg_tasks(sess, &waiting);
        let n = ag.bg.len();
        ag.activity = format!(
            "turn ended · {n} task{} in background",
            if n != 1 { "s" } else { "" }
        );
    }
    if ag.status.is_live() && !ag.question_ids.is_empty() {
        ag.status = Status::Waiting;
    }
    let label = run
        .and_then(|run| sess.workflows().started(run, &ag.id))
        .map_or("", |j| j.label);
    let fallback = ag.agent_type.as_deref().filter(|t| !t.is_empty());
    ag.role = role_of(&[label, &ag.description], fallback);
    if ag.status == Status::Running && ag.role == "reviewer" {
        ag.status = Status::Reviewing;
    }
    if ag.status.is_live()
        && ag.status != Status::Background
        && let Some(l) = truthy_time(last)
        && cx.now - l > QUIET_AFTER
    {
        let rest = if ag.activity.is_empty() {
            String::new()
        } else {
            format!(" · {}", ag.activity)
        };
        ag.activity = format!("quiet {}{rest}", dur(Some(cx.now - l)));
    }
}

/// The agents shown, by seq (sorted ids give the seq). An agent outside the window still
/// shows while an agent it spawned does, and so does every agent between them.
pub fn agents_of(cx: &Ctx) -> Arena {
    let mut out = Arena::new();
    let mut old: HashMap<String, usize> = HashMap::new();
    let mut ids: Vec<&str> = cx.sess.agents().map(|(id, _)| id).collect();
    ids.sort_unstable();
    for (seq, aid) in ids.into_iter().enumerate() {
        match build(cx, seq, aid, false) {
            Some(ag) => {
                out.insert(aid.to_owned(), ag);
            }
            None => {
                old.insert(aid.to_owned(), seq);
            }
        }
    }
    let shown: Vec<String> = out.keys().cloned().collect();
    for aid in shown {
        let mut seen: HashSet<String> = HashSet::from([aid.clone()]);
        let mut cur = cx.sess.parentage().parent_of(&aid);
        while let Some(Owner::Agent(c)) = cur {
            if !seen.insert(c.clone()) {
                break;
            }
            if let Some(&seq) = old.get(&c)
                && !out.contains_key(&c)
                && let Some(ag) = build(cx, seq, &c, true)
            {
                out.insert(c.clone(), ag);
            }
            cur = cx.sess.parentage().parent_of(&c);
        }
    }
    out.sort_by(|_, a, _, b| a.seq.cmp(&b.seq));
    out
}
