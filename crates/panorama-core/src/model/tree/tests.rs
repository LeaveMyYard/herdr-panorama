use super::*;
use crate::model::Status;

fn agent(id: &str, parent: Option<&str>) -> Agent {
    Agent {
        id: id.into(),
        parent: parent.map(Into::into),
        status: Status::Running,
        ..Agent::default()
    }
}

fn root(agents: Vec<Agent>) -> Root {
    Root {
        agents,
        ..serde_json::from_str(
            r#"{"pane_id": "p", "tab_id": null, "tab_label": "t", "session_id": null}"#,
        )
        .unwrap()
    }
}

fn ids(agents: &[&Agent]) -> Vec<String> {
    agents.iter().map(|a| a.id.clone()).collect()
}

#[test]
fn agents_group_under_the_agent_that_spawned_them() {
    let t = root(vec![
        agent("a", None),
        agent("b", Some("a")),
        agent("c", Some("a")),
        agent("d", Some("b")),
    ]);
    let k = TreeKids::of(&t);
    assert_eq!(ids(k.children(None)), ["a"]);
    assert_eq!(ids(k.children(Some("a"))), ["b", "c"]);
    assert_eq!(ids(k.children(Some("b"))), ["d"]);
    assert!(k.children(Some("d")).is_empty());
    assert!(k.has_children("a") && !k.has_children("c"));
}

// tests/test_deep.py::test_a_parent_loop_or_a_missing_parent_never_hides_an_agent
#[test]
fn a_parent_loop_or_a_missing_parent_never_hides_an_agent() {
    let t = root(vec![
        agent("a", Some("b")),
        agent("b", Some("a")),
        agent("c", Some("gone")),
        agent("d", Some("a")),
    ]);
    let k = TreeKids::of(&t);
    // c is a root child (its parent is not the root's); the loop a <-> b and d, which only
    // the loop reaches, join it in the root's order
    assert_eq!(ids(k.children(None)), ["c", "a", "b", "d"]);
    assert!(k.children(Some("a")).is_empty() && k.children(Some("b")).is_empty());
}

#[test]
fn an_agent_that_names_itself_is_a_root_child() {
    let t = root(vec![agent("a", Some("a")), agent("b", Some("a"))]);
    let k = TreeKids::of(&t);
    assert_eq!(ids(k.children(None)), ["a"]);
    assert_eq!(ids(k.children(Some("a"))), ["b"]);
}
