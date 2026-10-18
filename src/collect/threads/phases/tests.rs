use super::*;
use crate::collect::threads::spans::own_span;
use panorama_core::model::Agent;

fn agent(id: &str, desc: &str, role: &str, started: f64, status: Status) -> Agent {
    Agent {
        id: id.into(),
        description: desc.into(),
        role: role.into(),
        started: Some(started),
        status,
        ..Agent::default()
    }
}

fn arena(agents: Vec<Agent>) -> Arena {
    agents.into_iter().map(|a| (a.id.clone(), a)).collect()
}

#[test]
fn phases_number_review_rounds_and_repeat_other_words() {
    let mut ar = arena(vec![
        agent("a", "", "author", 1.0, Status::Done),
        agent("b", "", "reviewer", 2.0, Status::Done),
        agent("c", "", "author", 3.0, Status::Cancelled),
        agent("d", "", "reviewer", 4.0, Status::Failed),
    ]);
    for (id, end) in [("a", 1.5), ("b", 2.5), ("c", 3.5), ("d", 4.5)] {
        ar[id].ended = Some(end);
    }
    // a failed agent is live (Python `live`): its own span stays open; a notice ends it failed
    let mut spans: Vec<(String, Span)> = ["a", "b", "c"]
        .iter()
        .map(|id| (id.to_string(), own_span(&ar[*id], ar[*id].started)))
        .collect();
    assert_eq!(
        own_span(&ar["d"], Some(4.0)),
        Span {
            start: Some(4.0),
            end: None,
            outcome: Status::Running
        }
    );
    spans.push((
        "d".into(),
        Span {
            start: Some(4.0),
            end: Some(4.5),
            outcome: Status::Failed,
        },
    ));
    let phases = heuristic_phases(spans, &mut ar);
    let got: Vec<_> = phases
        .iter()
        .map(|p| (p.name.as_str(), p.state.code(), p.start, p.end))
        .collect();
    assert_eq!(
        got,
        [
            ("author", "done", Some(1.0), Some(1.5)),
            ("review r1", "done", Some(2.0), Some(2.5)),
            ("author 2", "skipped", Some(3.0), Some(3.5)),
            ("review r2", "failed", Some(4.0), Some(4.5)),
        ]
    );
}
