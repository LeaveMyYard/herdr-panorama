use super::*;

#[test]
fn exact_thread_binding_rejects_group_ids_old_cache_and_child_roots() {
    let observation = observed();
    for id in ["other-thread", "shared-session-group", ""] {
        let mut pane = pane(id);
        apply(&mut pane, Some(&observation), &channel());
        assert!(pane.threads.is_empty(), "{id}");
        assert_eq!(pane.session_id.as_deref(), Some(id));
        assert_eq!(pane.last_activity, None);
    }
    let mut child_root = observation;
    child_root.root.parent_thread_id = Some("another-root".into());
    let mut pane = pane("root");
    apply(&mut pane, Some(&child_root), &channel());
    assert!(pane.threads.is_empty());
}

#[test]
fn exact_children_keep_parents_and_depth_with_no_duplicate_root() {
    let observation = observed();
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel());
    let agents = &pane.threads[0].agents;
    assert_eq!(agents.len(), 3);
    assert_eq!(agents[0].id, "main:codex:w1:p1");
    assert_eq!(agents[0].depth, 0);
    assert_eq!(agents[1].id, "child");
    assert_eq!(agents[1].parent.as_deref(), Some("main:codex:w1:p1"));
    assert_eq!(agents[1].depth, 1);
    assert_eq!(agents[2].parent.as_deref(), Some("child"));
    assert_eq!(agents[2].depth, 2);
    assert_eq!(pane.session_id.as_deref(), Some("root"));
}

#[test]
fn disconnected_cyclic_duplicate_and_root_children_are_rejected() {
    let mut observation = observed();
    observation.children.extend([
        thread("orphan", Some("absent")),
        thread("cycle-a", Some("cycle-b")),
        thread("cycle-b", Some("cycle-a")),
        thread("root", Some("root")),
        thread("duplicate", Some("root")),
        thread("duplicate", Some("child")),
        thread("duplicate-child", Some("duplicate")),
    ]);
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel());
    assert_eq!(
        pane.threads[0]
            .agents
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["main:codex:w1:p1", "child", "grandchild"]
    );
}

#[test]
fn unavailable_observations_keep_herdr_metadata_without_old_detail() {
    let mut pane = pane("root");
    pane.tab_no = Some(2);
    pane.focused = true;
    pane.cwd = Some("/synthetic/shared".into());
    apply(&mut pane, Some(&observed()), &channel());
    assert!(!pane.threads.is_empty());
    apply(&mut pane, None, &channel());
    assert_eq!(pane.session_id.as_deref(), Some("root"));
    assert_eq!(pane.tab_no, Some(2));
    assert!(pane.focused);
    assert_eq!(pane.cwd.as_deref(), Some("/synthetic/shared"));
    assert!(pane.threads.is_empty());
    assert_eq!(pane.last_activity, None);
}
