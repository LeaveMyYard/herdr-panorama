//! An agent's spans (Python `collect.agent_spans`): one per launch and resume, each with its
//! start, end and outcome, from the session's notices and resumes.

use herdr_agent_kit::agent_session::Ts;
use herdr_agent_kit::agent_session::claude::Session;
use herdr_agent_kit::agent_session::claude::wire::Ending;
use panorama_core::model::{Agent, Status};

/// One launch or resume of an agent: start, end, outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub outcome: Status,
}

fn ending_status(e: Ending) -> Status {
    match e {
        Ending::Failed => Status::Failed,
        Ending::Cancelled => Status::Cancelled,
        _ => Status::Done,
    }
}

/// Its spans: one per launch and resume, each ended by the notice that falls before the next
/// start (else by that start, done), the last one open while the agent is live.
pub fn agent_spans(sess: &Session, a: &Agent) -> Vec<Span> {
    let mut starts: Vec<f64> = a
        .started
        .into_iter()
        .chain(sess.evidence().resumes(&a.id).map(Ts::secs))
        .collect();
    starts.sort_by(f64::total_cmp);
    starts.dedup();
    let mut ends: Vec<(f64, Ending)> = sess
        .evidence()
        .notices(&a.id)
        .map(|n| (n.at.0, n.ending))
        .collect();
    ends.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut out = Vec::new();
    for (i, &s) in starts.iter().enumerate() {
        let nxt = starts.get(i + 1).copied().unwrap_or(f64::INFINITY);
        let span = if let Some((t, e)) = ends.iter().find(|(t, _)| s <= *t && *t < nxt) {
            Span {
                start: Some(s),
                end: Some(*t),
                outcome: ending_status(*e),
            }
        } else if i + 1 < starts.len() {
            Span {
                start: Some(s),
                end: Some(nxt),
                outcome: Status::Done,
            }
        } else {
            own_span(a, Some(s))
        };
        out.push(span);
    }
    out
}

/// The span the agent's own fields give.
pub fn own_span(a: &Agent, start: Option<f64>) -> Span {
    if a.status.is_live() {
        Span {
            start,
            end: None,
            outcome: Status::Running,
        }
    } else {
        Span {
            start,
            end: a.ended,
            outcome: a.status.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
