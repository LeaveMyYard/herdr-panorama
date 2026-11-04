use super::*;

#[test]
fn exact_thread_binding_rejects_group_ids_old_cache_and_child_roots() {
    let observation = observed();
    for id in ["other-thread", "shared-session-group", ""] {
        let mut pane = pane(id);
        assert!(
            apply(&mut pane, Some(&observation)).agents.is_empty(),
            "{id}"
        );
        assert_eq!(pane.session_id.as_deref(), Some(id));
        assert_eq!(pane.last_activity, None);
    }
    let mut child_root = observation;
    child_root.root.parent_thread_id = Some("another-root".into());
    assert!(
        apply(&mut pane("root"), Some(&child_root))
            .agents
            .is_empty()
    );
}

#[test]
fn exact_children_keep_parents_and_depth_with_no_root_agent() {
    let observation = observed();
    let mut pane = pane("root");
    let tree = apply(&mut pane, Some(&observation));
    let agents = &tree.agents;
    assert_eq!(ids(&tree), ["child", "grandchild"]);
    // the root's own child has no parent: it is the root's
    assert_eq!(agents[0].parent, None);
    assert_eq!(agents[0].depth, 1);
    assert_eq!(agents[1].parent.as_deref(), Some("child"));
    assert_eq!(agents[1].depth, 2);
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
    let tree = apply(&mut pane("root"), Some(&observation));
    assert_eq!(ids(&tree), ["child", "grandchild"]);
}

#[test]
fn unavailable_observations_keep_herdr_metadata_without_old_detail() {
    let mut pane = pane("root");
    pane.tab_no = Some(2);
    pane.focused = true;
    pane.cwd = Some("/synthetic/shared".into());
    assert!(!apply(&mut pane, Some(&observed())).agents.is_empty());
    let tree = apply(&mut pane, None);
    assert_eq!(pane.session_id.as_deref(), Some("root"));
    assert_eq!(pane.tab_no, Some(2));
    assert!(pane.focused);
    assert_eq!(pane.cwd.as_deref(), Some("/synthetic/shared"));
    assert!(tree.agents.is_empty());
    assert_eq!(pane.last_activity, None);
}
