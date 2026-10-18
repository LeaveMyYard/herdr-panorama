//! A native Workflow run as a thread (Python `collect.workflow_thread`,
//! `threads.workflow_phases`): its agents, its phases in the summary's order (then the
//! journal's and the agents' own), and its status from the run's end.

use super::Draft;
use crate::collect::agents::Arena;
use crate::collect::py::{or_time, py_str, str_of, truthy, truthy_str, truthy_time};
use herdr_agent_kit::agent_session::Ts;
use herdr_agent_kit::agent_session::claude::Session;
use herdr_agent_kit::agent_session::claude::fold::TaskEnd;
use herdr_agent_kit::agent_session::claude::wire::Ending;
use indexmap::IndexMap;
use panorama_core::model::{Agent, Phase, PhaseState, Status, ThreadSource, ThreadStatus};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// The phases of `order`, each with the agents whose phase it is (they get its index).
fn workflow_phases(
    order: &[String],
    ids: &[String],
    phase_of: &IndexMap<String, Option<String>>,
    run_live: bool,
    detail: &HashMap<String, String>,
    arena: &mut Arena,
) -> Vec<Phase> {
    let mut phases = Vec::new();
    for (i, title) in order.iter().enumerate() {
        let mine: Vec<&String> = ids
            .iter()
            .filter(|id| phase_of.get(*id).and_then(Option::as_deref) == Some(title.as_str()))
            .collect();
        for id in &mine {
            let a = &mut arena[*id];
            if !a.phases.contains(&(i as i64)) {
                a.phases.push(i as i64);
            }
        }
        let ags: Vec<&Agent> = mine.iter().map(|id| &arena[*id]).collect();
        let start = ags
            .iter()
            .filter_map(|a| truthy_time(a.started))
            .reduce(f64::min);
        let end = ags
            .iter()
            .filter_map(|a| or_time(a.ended, a.last_at))
            .filter(|e| *e != 0.0)
            .reduce(f64::max);
        let state = if ags.is_empty() {
            if run_live {
                PhaseState::Next
            } else {
                PhaseState::Skipped
            }
        } else if ags.iter().any(|a| a.status.is_live()) {
            PhaseState::Now
        } else {
            let newest = ags.iter().fold(None::<&&Agent>, |best, a| match best {
                Some(b) if b.ended.unwrap_or(0.0) >= a.ended.unwrap_or(0.0) => Some(b),
                _ => Some(a),
            });
            if newest.is_some_and(|n| n.status == Status::Failed) {
                PhaseState::Failed
            } else {
                PhaseState::Done
            }
        };
        let end = if state == PhaseState::Now { None } else { end };
        let note = detail.get(title).filter(|d| !d.is_empty()).cloned();
        phases.push(Phase {
            name: title.clone(),
            state,
            start,
            end,
            note,
        });
    }
    phases
}

/// The summary's phases that have a title.
fn summary_phases(summary: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    summary
        .get("phases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter(|p| p.get("title").is_some_and(truthy))
        .collect()
}

/// The run's thread, or `None` when it has no agent left to show and is not running.
pub fn workflow_thread(
    sess: &Session,
    run: &str,
    arena: &mut Arena,
    claimed: &mut HashSet<String>,
) -> Option<Draft> {
    let ids: Vec<String> = sess
        .agents()
        .filter(|(id, r)| {
            r.workflow.as_deref() == Some(run) && arena.contains_key(*id) && !claimed.contains(*id)
        })
        .map(|(id, _)| id.to_owned())
        .collect();
    let live_run = sess.workflows().live(run);
    if ids.is_empty() && live_run != Some(true) {
        return None;
    }
    claimed.extend(ids.iter().cloned());
    let empty = Map::new();
    let summary = sess.workflows().summary(run).unwrap_or(&empty);
    let mut phase_of: IndexMap<String, Option<String>> = IndexMap::new();
    for id in &ids {
        let journal = sess
            .workflows()
            .started(run, id)
            .and_then(|j| j.phase)
            .filter(|p| !p.is_empty());
        let meta = sess.agent(id).map(|r| &r.meta);
        let meta_phase = meta.and_then(|m| str_of(m, "workflowPhase"));
        phase_of.insert(id.clone(), journal.or(meta_phase).map(str::to_owned));
    }
    let titled = summary_phases(summary);
    let mut order: Vec<String> = titled.iter().map(|p| py_str(&p["title"])).collect();
    let extra: Vec<String> = sess
        .workflows()
        .phases(run)
        .map(str::to_owned)
        .chain(
            phase_of
                .values()
                .flatten()
                .filter(|p| !p.is_empty())
                .cloned(),
        )
        .collect();
    for ph in extra {
        if !order.contains(&ph) {
            order.push(ph);
        }
    }
    let detail: HashMap<String, String> = titled
        .iter()
        .map(|p| {
            (
                py_str(&p["title"]),
                truthy_str(p, "detail").unwrap_or_default(),
            )
        })
        .collect();
    let mut launches: Vec<_> = sess
        .workflows()
        .launches()
        .filter(|w| w.run_id == run)
        .collect();
    launches.sort_by(|a, b| {
        a.ts.map_or(0.0, Ts::secs)
            .total_cmp(&b.ts.map_or(0.0, Ts::secs))
    });
    let name = launches
        .iter()
        .rev()
        .find_map(|w| w.name.clone().filter(|n| !n.is_empty()))
        .or_else(|| truthy_str(summary, "workflowName"))
        .unwrap_or_else(|| run.to_owned());
    let run_is_live = live_run.unwrap_or_else(|| ids.iter().any(|id| arena[id].status.is_live()));
    let mut d = Draft::new(format!("wf:{run}"), name, ThreadSource::Workflow);
    d.thread.flavour = Some("workflow".into());
    d.thread.phases = workflow_phases(&order, &ids, &phase_of, run_is_live, &detail, arena);
    d.ids = ids;
    let end = sess.workflows().end(run);
    d.thread.status = match live_run {
        None => ThreadStatus::from_code(""),
        Some(true) => ThreadStatus::Active,
        Some(false) => {
            d.thread.ended = end.map(|e| e.at.0);
            match end {
                Some(TaskEnd {
                    ending: Ending::Failed,
                    ..
                }) => ThreadStatus::Failed,
                Some(TaskEnd {
                    ending: Ending::Cancelled,
                    ..
                }) => ThreadStatus::Cancelled,
                _ => ThreadStatus::Done,
            }
        }
    };
    d.thread.started = launches.first().and_then(|w| w.ts).map(Ts::secs);
    Some(d)
}
