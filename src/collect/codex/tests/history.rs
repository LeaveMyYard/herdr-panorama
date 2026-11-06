use super::*;

#[test]
fn the_root_keeps_its_runtime_and_history_never_completes_a_child() {
    let observation = observed();
    for runtime in [
        HerdrStatus::Idle,
        HerdrStatus::Working,
        HerdrStatus::Blocked,
        HerdrStatus::Unknown,
        HerdrStatus::Done,
    ] {
        let mut pane = pane("root");
        pane.herdr_status = runtime.clone();
        let tree = apply(&mut pane, Some(&observation));
        assert_eq!(pane.herdr_status, runtime);
        // the root thread is the root itself: no agent stands for it
        assert_eq!(ids(&tree), ["child", "grandchild"]);
        for a in &tree.agents {
            assert_eq!(a.status, Status::from_code("unknown"));
            assert_eq!(a.ended, None);
        }
        assert_eq!(tree.hidden, 0);
        assert_eq!(pane.last_activity, Some(30.0));
    }
}

#[test]
fn latest_tool_cannot_leak_from_an_older_turn_and_text_is_bounded() {
    let mut observation = observed();
    let turn = &mut observation.children[0].turns[0];
    turn.latest_tool.as_mut().unwrap().turn_id = "old-turn".into();
    turn.summary = "🦀".repeat(900);
    turn.prompt = "🦀".repeat(900);
    let tree = apply(&mut pane("root"), Some(&observation));
    let child = &tree.agents[0];
    assert_eq!(child.id, "child");
    assert!(child.activity.is_empty());
    assert_eq!(child.tool_calls, 2);
    assert_eq!(child.last_message.chars().count(), 500);
}

#[test]
fn a_failed_child_turn_is_visible_without_inventing_lifecycle_state() {
    let mut observation = observed();
    observation.children[0].turns[0].status = TurnStatus::Failed;
    let tree = apply(&mut pane("root"), Some(&observation));
    let child = &tree.agents[0];
    assert_eq!(child.id, "child");
    assert_eq!(child.status, Status::from_code("unknown"));
    assert_eq!(child.last_message, "failed turn: Synthetic result");
    assert_eq!(child.ended, None);
}

#[test]
fn an_unnamed_thread_is_titled_by_its_first_prompt_not_the_read_window() {
    let mut observation = observed();
    observation.root.name = None;
    observation.children[0].name = None;
    observation.children[1].name = None;
    observation.root.preview = Some("Ship the billing export\nthen tidy up".into());
    observation.children[0].preview = Some("Review the diff".into());
    let mut pane = pane("root");
    let (seen, tree) = enrich(&mut pane, Some(&observation));
    assert_eq!(seen.name, None);
    assert_eq!(
        seen.prompt.as_deref(),
        Some("Ship the billing export\nthen tidy up")
    );
    let title = |id: &str| {
        let a = tree.agents.iter().find(|a| a.id == id).unwrap();
        (a.title.clone(), a.title_source.clone())
    };
    assert_eq!(title("child"), ("Review the diff".into(), "prompt".into()));
    // Without a preview, the earliest prompt read names it.
    assert_eq!(
        title("grandchild"),
        ("Synthetic task".into(), "prompt".into())
    );
}
