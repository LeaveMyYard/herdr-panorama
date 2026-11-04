use super::*;

fn agent(id: &str, parent: Option<&str>, status: Status, started: f64) -> Agent {
    Agent {
        id: id.into(),
        parent: parent.map(str::to_owned),
        status,
        started: Some(started),
        depth: 7, // advisory: the chain decides
        ..Agent::default()
    }
}

fn shape(tree: &Tree) -> Vec<(&str, Option<&str>, i64)> {
    tree.agents
        .iter()
        .map(|a| (a.id.as_str(), a.parent.as_deref(), a.depth))
        .collect()
}

#[test]
fn parents_come_before_children_and_siblings_most_urgent_then_oldest() {
    let tree = Tree::new(
        [
            agent("c2", Some("b"), Status::Running, 5.0),
            agent("b", Some("a"), Status::Done, 2.0),
            agent("a", None, Status::Done, 1.0),
            agent("c1", Some("b"), Status::Running, 4.0),
            agent("w", None, Status::Waiting, 9.0),
            agent("d", Some("c1"), Status::Failed, 6.0),
        ],
        3,
    );
    assert_eq!(
        shape(&tree),
        [
            ("w", None, 1),
            ("a", None, 1),
            ("b", Some("a"), 2),
            ("c1", Some("b"), 3),
            ("d", Some("c1"), 4),
            ("c2", Some("b"), 3),
        ]
    );
    assert_eq!(tree.hidden, 3);
}

#[test]
fn a_dangling_self_or_cyclic_parent_makes_a_root_child_and_hides_nobody() {
    let tree = Tree::new(
        [
            agent("lost", Some("gone"), Status::Running, 1.0),
            agent("me", Some("me"), Status::Running, 2.0),
            agent("x", Some("y"), Status::Running, 3.0),
            agent("y", Some("x"), Status::Running, 4.0),
            agent("under", Some("y"), Status::Running, 5.0),
        ],
        0,
    );
    assert_eq!(
        shape(&tree),
        [
            ("lost", None, 1),
            ("me", None, 1),
            ("y", None, 1),
            ("x", Some("y"), 2),
            ("under", Some("y"), 2),
        ]
    );
}

#[test]
fn every_root_carries_one_thread_of_all_its_agents() {
    let mut o = Orchestrator {
        title: "Ship it".into(),
        hidden_done: 4,
        flavour: "workflow".into(),
        ..serde_json::from_value(serde_json::json!({"pane_id": "p", "tab_id": null,
            "tab_label": "t", "session_id": null}))
        .unwrap()
    };
    let mut done = agent("a", None, Status::Done, 1.0);
    done.ended = Some(8.0);
    let mut failed = agent("b", Some("a"), Status::Failed, 2.0);
    failed.last_at = Some(9.0);
    adapt(&mut o, Tree::new([done, failed], 2));
    let [t] = o.threads.as_slice() else {
        panic!("one thread: {:?}", o.threads)
    };
    assert!(t.is_tree());
    assert_eq!((t.name.as_str(), t.phases.len()), ("Ship it", 0));
    assert_eq!(t.agents.len(), 2);
    // the newest (by its last record) failed
    assert_eq!(t.status, ThreadStatus::Failed);
    assert_eq!((t.started, t.ended), (Some(1.0), Some(9.0)));
    assert_eq!((o.hidden_finished, o.hidden_done), (2, 0));
    assert_eq!(o.flavour, "agents");
    // a root with no agents still has its one thread, empty and done
    adapt(&mut o, Tree::default());
    assert_eq!(o.threads.len(), 1);
    assert!(o.threads[0].agents.is_empty());
    assert_eq!(o.threads[0].status, ThreadStatus::Done);
    assert_eq!(o.hidden_finished, 0);
}
