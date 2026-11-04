use super::*;
use crate::model::ThreadSource;

fn agent(id: &str, st: Status) -> Agent {
    Agent {
        id: id.into(),
        status: st,
        ..Agent::default()
    }
}

fn names(st: &OrchState) -> (Status, &str, &str) {
    (st.st.clone(), st.glyph.as_str(), st.label.as_str())
}

#[test]
fn attention_wins_and_says_working_alongside() {
    let (a, b) = (agent("a", Status::Running), agent("b", Status::Waiting));
    let st = state_of(&[&a, &b], false, 0.0, &Extras::default(), None);
    assert_eq!(names(&st), (Status::Waiting, "?", "needs you · working"));
    assert!(st.working);
    let f = agent("f", Status::Failed);
    let st = state_of(&[&b, &f], false, 0.0, &Extras::default(), None);
    assert_eq!(names(&st), (Status::Failed, "✗", "failed"));
}

#[test]
fn the_ladder_below_attention() {
    let none = Extras::default();
    let run = agent("r", Status::Running);
    assert_eq!(
        names(&state_of(&[&run], false, 1.0, &none, None)),
        (Status::Running, "⠙", "working")
    );
    let rev = agent("v", Status::Reviewing);
    assert_eq!(
        names(&state_of(&[&rev], false, 0.0, &none, None)),
        (Status::Reviewing, "»", "reviewing")
    );
    let done = agent("d", Status::Done);
    assert_eq!(
        names(&state_of(&[&done], true, 0.0, &none, None)),
        (Status::Done, "✓", "all done")
    );
    assert_eq!(
        names(&state_of(&[&done], false, 0.0, &none, None)),
        (Status::Queued, "○", "idle")
    );
    let stale = Extras {
        stale_label: Some("stale 13h".into()),
        ..Extras::default()
    };
    // stale wins over done (turn 8)
    assert_eq!(
        names(&state_of(&[&done], true, 0.0, &stale, None)),
        (Status::Stale, "◌", "stale 13h")
    );
}

#[test]
fn waiting_says_how_long_past_stale_after() {
    let bg = [BgTask {
        id: "b".into(),
        kind: "Bash".into(),
        description: String::new(),
        started: Some(0.0),
    }];
    let x = Extras {
        bg: &bg,
        ..Extras::default()
    };
    let short = Ages {
        now: 600.0,
        stale_after: 3600.0,
    };
    let st = state_of(&[], true, 0.0, &x, Some(short));
    assert_eq!(
        (names(&st), st.bg),
        ((Status::Background, "⠁", "waiting"), 1)
    );
    let long = Ages {
        now: 7200.0,
        stale_after: 3600.0,
    };
    assert_eq!(state_of(&[], true, 0.0, &x, Some(long)).label, "waiting 2h");
}

#[test]
fn an_orchestrator_counts_herdrs_word_and_its_questions() {
    let mut o: Orchestrator = serde_json::from_value(serde_json::json!(
        {"pane_id": "p", "tab_id": null, "tab_label": "x", "session_id": null,
         "herdr_status": "working", "last_activity": 0.0}))
    .unwrap();
    assert_eq!(orch_state(&o, 0.0, None).label, "working");
    o.inbox = vec![1];
    assert_eq!(orch_state(&o, 0.0, None).label, "needs you · working");
    o.inbox.clear();
    o.herdr_status = HerdrStatus::Idle;
    let ages = Ages {
        now: 50_000.0,
        stale_after: 43_200.0,
    };
    assert!(ages.is_stale(&o));
    assert_eq!(
        names(&orch_state(&o, 0.0, Some(ages))),
        (Status::Stale, "◌", "stale 13h")
    );
    o.threads.push(
        serde_json::from_value(serde_json::json!(
        {"id": "t", "name": "n", "source": "turn", "status": "done"}))
        .unwrap(),
    );
    assert_eq!(o.threads[0].source, ThreadSource::Turn);
    assert_eq!(orch_state(&o, 0.0, None).label, "all done");
}

#[test]
fn bare_codex_preserves_authoritative_runtime_without_inventing_completion() {
    let legacy: Orchestrator = serde_json::from_value(serde_json::json!({
        "pane_id": "p", "tab_id": null, "tab_label": "x", "session_id": null,
        "herdr_status": "unknown", "last_activity": null
    }))
    .unwrap();
    for session_id in [None, Some("bound-but-history-unavailable".to_owned())] {
        let mut o = Orchestrator {
            harness: "codex".into(),
            session_id,
            ..legacy.clone()
        };
        for (runtime, expected, label) in [
            (HerdrStatus::Done, Status::Done, "all done"),
            (HerdrStatus::Idle, Status::Queued, "idle"),
            (
                HerdrStatus::Unknown,
                Status::Unknown("unknown".into()),
                "unknown",
            ),
            (HerdrStatus::Working, Status::Running, "working"),
            (HerdrStatus::Blocked, Status::Blocked, "blocked"),
        ] {
            o.herdr_status = runtime;
            let state = orch_state(&o, 0.0, None);
            assert_eq!((state.st, state.label.as_str()), (expected, label));
        }
        o.herdr_status = HerdrStatus::Done;
        o.inbox.push(1);
        assert_eq!(orch_state(&o, 0.0, None).st, Status::Waiting);
        o.inbox.clear();
        o.bg.push(BgTask {
            id: "background".into(),
            kind: "task".into(),
            description: String::new(),
            started: None,
        });
        assert_eq!(orch_state(&o, 0.0, None).st, Status::Background);
    }
    // The pre-existing Claude empty-session display remains compatible.
    assert_eq!(orch_state(&legacy, 0.0, None).st, Status::Done);
}

#[test]
fn codex_history_age_does_not_replace_root_runtime_but_a_live_child_does() {
    // the uniform tree (U3): the root's runtime is herdr's, on the root; no main agent
    let mut o: Orchestrator = serde_json::from_value(serde_json::json!({
        "pane_id":"p", "tab_id":null, "tab_label":"Codex", "session_id":"root",
        "harness":"codex", "herdr_status":"done", "last_activity":1,
        "threads":[{"id":crate::model::TREE_THREAD, "name":"agents", "source":"session",
            "status":"active", "agents":[{"id":"child", "status":"unknown"}]}]
    }))
    .unwrap();
    let ages = Some(Ages {
        now: 1000.0,
        stale_after: 10.0,
    });
    assert_eq!(orch_state(&o, 0.0, ages).st, Status::Done);
    o.herdr_status = HerdrStatus::Unknown;
    assert_eq!(
        orch_state(&o, 0.0, ages).st,
        Status::Unknown("unknown".into())
    );
    o.herdr_status = HerdrStatus::Idle;
    assert_eq!(orch_state(&o, 0.0, ages).st, Status::Stale);
    o.herdr_status = HerdrStatus::Done;
    o.threads[0].agents[0].status = Status::Running;
    assert_eq!(orch_state(&o, 0.0, ages).st, Status::Running);
}

#[test]
fn a_root_question_no_agent_filed_keeps_an_idle_finished_root_needing_you() {
    // the uniform tree (U3): no session card carries the question, the root does
    for agents in [vec![], vec![agent("d", Status::Done)]] {
        let o: Orchestrator = serde_json::from_value(serde_json::json!({
            "pane_id": "p", "tab_id": null, "tab_label": "t", "session_id": null,
            "herdr_status": "idle", "inbox": [7], "inbox_texts": ["Merge now?"],
            "threads": [{"id": crate::model::TREE_THREAD, "name": "t", "source": "session",
                         "status": "done", "agents": agents}]
        }))
        .unwrap();
        assert_eq!(
            names(&orch_state(&o, 0.0, None)),
            (Status::Waiting, "?", "needs you")
        );
        let answered = Orchestrator {
            inbox: vec![],
            inbox_texts: vec![],
            ..o
        };
        assert_eq!(names(&orch_state(&answered, 0.0, None)).0, Status::Done);
    }
}
