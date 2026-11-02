use super::*;

#[test]
fn persisted_completion_never_completes_an_idle_or_working_session() {
    let observation = observed();
    for (runtime, expected) in [
        (HerdrStatus::Idle, Status::Queued),
        (HerdrStatus::Working, Status::Running),
        (HerdrStatus::Blocked, Status::Blocked),
        (HerdrStatus::Unknown, Status::from_code("unknown")),
        (HerdrStatus::Done, Status::Done),
    ] {
        let mut pane = pane("root");
        pane.herdr_status = runtime.clone();
        apply(&mut pane, Some(&observation), &channel());
        assert_eq!(pane.herdr_status, runtime);
        let session = &pane.threads[0];
        assert_eq!(session.agents[0].status, expected);
        assert_eq!(
            session.status == ThreadStatus::Done,
            runtime == HerdrStatus::Done
        );
        assert_eq!(session.phases[0].state, PhaseState::Done);
        assert_eq!(session.agents[1].status, Status::from_code("unknown"));
        assert_eq!(session.agents[1].ended, None);
    }
}

#[test]
fn turns_are_chronological_and_unresolved_history_needs_live_evidence() {
    let mut observation = observed();
    observation.root.turns = [
        TurnStatus::InProgress,
        TurnStatus::Unknown,
        TurnStatus::Interrupted,
        TurnStatus::Failed,
        TurnStatus::Completed,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, status)| Turn {
        id: format!("turn-{i}"),
        status,
        prompt: format!("prompt-{i}"),
        ..Turn::default()
    })
    .collect();
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel());
    let phases = &pane.threads[0].phases;
    assert_eq!(
        phases.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["prompt-4", "prompt-3", "prompt-2", "prompt-1", "prompt-0"]
    );
    assert_eq!(
        phases.iter().map(|p| p.state.clone()).collect::<Vec<_>>(),
        [
            PhaseState::Done,
            PhaseState::Failed,
            PhaseState::Skipped,
            PhaseState::from_code("unknown"),
            PhaseState::Now,
        ]
    );
    pane.herdr_status = HerdrStatus::Idle;
    apply(&mut pane, Some(&observation), &channel());
    assert_eq!(
        pane.threads[0].phases[4].state,
        PhaseState::from_code("unknown")
    );
}

#[test]
fn latest_tool_cannot_leak_from_an_older_turn_and_text_is_bounded() {
    let mut observation = observed();
    observation.root.turns[0]
        .latest_tool
        .as_mut()
        .unwrap()
        .turn_id = "old-turn".into();
    observation.root.turns[0].summary = "🦀".repeat(900);
    observation.root.turns[0].prompt = "🦀".repeat(900);
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel());
    let session = &pane.threads[0];
    assert!(session.agents[0].activity.is_empty());
    assert_eq!(session.agents[0].tool_calls, 2);
    assert_eq!(session.agents[0].last_message.chars().count(), 500);
    assert_eq!(session.phases[0].name.chars().count(), 240);
    assert_eq!(
        session.phases[0].note.as_ref().unwrap().chars().count(),
        500
    );
}

#[test]
fn a_failed_child_turn_is_visible_without_inventing_lifecycle_state() {
    let mut observation = observed();
    observation.children[0].turns[0].status = TurnStatus::Failed;
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel());
    let child = &pane.threads[0].agents[1];
    assert_eq!(child.status, Status::from_code("unknown"));
    assert_eq!(child.last_message, "failed turn: Synthetic result");
    assert_eq!(child.ended, None);
}
