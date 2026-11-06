use super::*;
use crate::view::state::FIN_CARD;
use serde_json::{Value, json};

fn swarm(orchs: Value) -> Swarm {
    serde_json::from_value(json!({ "roots": orchs })).unwrap()
}

fn orch(pane: &str, extra: Value) -> Value {
    let mut o = json!({"pane_id": pane, "tab_id": null, "tab_label": pane, "session_id": null});
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    o
}

/// A root with the uniform tree's one thread holding `agents`.
fn root(pane: &str, agents: Value) -> Value {
    orch(pane, json!({"agents": agents}))
}

fn root_of(agents: Value) -> Root {
    serde_json::from_value(root("p", agents)).unwrap()
}

#[test]
fn columns_list_the_roots_then_the_selected_roots_tree_by_depth() {
    let sw = swarm(json!([
        root(
            "p1",
            json!([
                {"id": "a", "status": "run"},
                {"id": "a1", "status": "run", "parent": "a"},
                {"id": "f1", "status": "done"}, {"id": "f2", "status": "done"},
            ])
        ),
        orch("p2", json!({}))
    ]));
    let mut v = View {
        orch: Some("p1".into()),
        ..View::default()
    };
    let cols = nav(&sw, &v);
    assert_eq!(cols[0], ["p1", "p2", "new:"]);
    // depth 1: the live agent, then the root's own finished ones folded
    assert_eq!(cols[1], ["a".to_owned(), format!("{FIN_CARD}p1")]);
    assert_eq!(cols[2], ["a1"]);
    assert_eq!(cols.len(), 3);
    v.fin_open.insert("p1".into());
    let cols = nav(&sw, &v);
    assert_eq!(
        cols[1],
        [
            "a".to_owned(),
            format!("{FIN_CARD}p1"),
            "f1".into(),
            "f2".into()
        ]
    );
    // a root with no agents: an empty depth-1 column
    v.orch = Some("p2".into());
    assert_eq!(nav(&sw, &v), [vec!["p1", "p2", "new:"], vec![]]);
}

#[test]
fn every_thread_of_an_older_producer_is_one_tree() {
    let sw = swarm(json!([orch(
        "p1",
        json!({"agents": [{"id": "a", "status": "run"}, {"id": "b", "status": "run", "parent": "a"}]})
    )]));
    let v = View {
        orch: Some("p1".into()),
        ..View::default()
    };
    let cols = nav(&sw, &v);
    assert_eq!(
        (cols[1].clone(), cols[2].clone()),
        (vec!["a".to_owned()], vec!["b".to_owned()])
    );
}

#[test]
fn filters_narrow_the_tree() {
    let sw = swarm(json!([root(
        "p1",
        json!([
            {"id": "q", "status": "wait"},
            {"id": "r", "status": "run"},
            {"id": "x", "status": "fail"},
            {"id": "d", "status": "done"},
        ])
    )]));
    let mut v = View {
        orch: Some("p1".into()),
        ..View::default()
    };
    let fin = format!("{FIN_CARD}p1");
    assert_eq!(nav(&sw, &v)[1], ["q", "r", "x", fin.as_str()]);
    v.filter = crate::view::Filter::Running;
    assert_eq!(nav(&sw, &v)[1], ["q", "r", "x"]);
    v.filter = crate::view::Filter::Needs; // a failed agent needs the operator too
    assert_eq!(nav(&sw, &v)[1], ["q", "x"]);
}

#[test]
fn default_children_take_the_remembered_else_the_first() {
    let sw = swarm(json!([root(
        "p1",
        json!([{"id": "a", "status": "run"}, {"id": "b", "status": "run"}])
    )]));
    let mut v = View {
        orch: Some("p1".into()),
        ..View::default()
    };
    default_children(&sw, &mut v);
    assert_eq!(v.agent.as_deref(), Some("a"));
    v.remember_agent.insert("p1".into(), "b".into());
    default_children(&sw, &mut v);
    assert_eq!(v.agent.as_deref(), Some("b"));
    v.remember_agent.insert("p1".into(), "gone".into());
    default_children(&sw, &mut v);
    assert_eq!(v.agent.as_deref(), Some("a"));
}

#[test]
fn the_path_to_an_agent_runs_from_the_roots_own() {
    let o = root_of(json!([
        {"id": "a"}, {"id": "b", "parent": "a"}, {"id": "c", "parent": "b"},
    ]));
    assert_eq!(path_ids(Some(&o), Some("c")), ["a", "b", "c"]);
    assert!(path_ids(Some(&o), Some("x")).is_empty());
    assert!(path_ids(None, Some("c")).is_empty());
}

#[test]
fn an_agents_group_and_depth() {
    let o = root_of(json!([
        {"id": "a"}, {"id": "b", "parent": "a"}, {"id": "c", "parent": "b"},
    ]));
    assert_eq!(parent_key(&o, "a"), "p");
    assert_eq!(parent_key(&o, "c"), "b");
    assert_eq!(parent_key(&o, &format!("{FIN_CARD}b")), "b");
    assert_eq!(agent_depth(Some(&o), Some("c")), 3);
    assert_eq!(agent_depth(Some(&o), Some(&format!("{FIN_CARD}p"))), 1);
    assert_eq!(agent_depth(Some(&o), Some(&format!("{FIN_CARD}b"))), 3);
    assert_eq!(agent_depth(Some(&o), Some("x")), 1);
    assert_eq!(agent_depth(None, Some("c")), 1);
}

#[test]
fn a_finished_card_knows_what_it_folds() {
    let o = root_of(json!([
        {"id": "a", "status": "run"}, {"id": "d1", "status": "done"}, {"id": "d2", "status": "fail"},
    ]));
    let entries = tree_entries(&o, &View::default());
    let fin = entries
        .iter()
        .find(|e| e.ident.starts_with(FIN_CARD))
        .unwrap();
    assert_eq!(fin.fin, ["d1"]); // a failed agent stays shown
}
