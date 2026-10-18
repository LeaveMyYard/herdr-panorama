use super::*;

#[test]
fn a_live_agent_s_own_span_is_open_and_an_ended_one_keeps_its_outcome() {
    let mut a = Agent {
        id: "a".into(),
        started: Some(1.0),
        ended: Some(2.0),
        status: Status::Running,
        ..Agent::default()
    };
    assert_eq!(
        own_span(&a, Some(1.0)),
        Span {
            start: Some(1.0),
            end: None,
            outcome: Status::Running
        }
    );
    a.status = Status::Failed; // live, as Python's `live` says
    assert_eq!(own_span(&a, Some(1.0)).end, None);
    a.status = Status::Cancelled;
    assert_eq!(
        own_span(&a, Some(1.0)),
        Span {
            start: Some(1.0),
            end: Some(2.0),
            outcome: Status::Cancelled
        }
    );
}
