use super::*;
use crate::view::state::FIN_CARD;
use serde_json::{Value, json};

fn swarm(orchs: Value) -> Swarm {
    serde_json::from_value(json!({ "orchestrators": orchs })).unwrap()
}

fn orch(pane: &str, extra: Value) -> Value {
    let mut o = json!({"pane_id": pane, "tab_id": null, "tab_label": pane, "session_id": null});
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    o
}

fn th(id: &str, status: &str, agents: Value) -> Value {
    json!({"id": id, "name": id, "source": "turn", "status": status, "agents": agents})
}

#[test]
fn columns_list_the_threads_then_the_selected_threads_tree() {
    let sw = swarm(json!([
        orch(
            "p1",
            json!({
            "bg": [{"id": "b", "kind": "k"}],
            "threads": [
                th("t1", "active", json!([
                    {"id": "a", "status": "run"},
                    {"id": "a1", "status": "run", "parent": "a"},
                    {"id": "f1", "status": "done"}, {"id": "f2", "status": "done"},
                ])),
                th("t2", "done", json!([])),
                th("t3", "failed", json!([])),
            ]})
        ),
        orch("p2", json!({}))
    ]));
    let mut v = View {
        orch: Some("p1".into()),
        thread: Some("t1".into()),
        ..View::default()
    };
    let cols = nav(&sw, &v);
    assert_eq!(cols[0], ["p1", "p2", "new:"]);
    // the background card, active then failed threads, the done card
    assert_eq!(cols[1], ["bg:p1", "t1", "t3", "done:p1"]);
    assert_eq!(cols[2], ["a".to_owned(), format!("{FIN_CARD}t1")]);
    assert_eq!(cols[3], ["a1"]);
    v.done_open.insert("p1".into());
    v.fin_open.insert("t1".into());
    let cols = nav(&sw, &v);
    assert_eq!(cols[1], ["bg:p1", "t1", "t3", "done:p1", "t2"]);
    assert_eq!(
        cols[2],
        [
            "a".to_owned(),
            format!("{FIN_CARD}t1"),
            "f1".into(),
            "f2".into()
        ]
    );
}

#[test]
fn filters_narrow_the_threads() {
    let sw = swarm(json!([orch(
        "p1",
        json!({"threads": [
            th("asks", "active", json!([{"id": "q", "status": "wait"}])),
            th("works", "active", json!([{"id": "r", "status": "run"}])),
            th("broke", "failed", json!([])),
            th("over", "done", json!([])),
        ]})
    )]));
    let o = &sw.orchestrators[0];
    let ids = |(a, d): (Vec<&Thread>, Vec<&Thread>)| -> (Vec<String>, usize) {
        (a.iter().map(|t| t.id.clone()).collect(), d.len())
    };
    assert_eq!(
        ids(split_threads(o, Filter::All)),
        (vec!["asks".into(), "works".into(), "broke".into()], 1)
    );
    assert_eq!(
        ids(split_threads(o, Filter::Running)),
        (vec!["asks".into(), "works".into()], 0)
    );
    assert_eq!(
        ids(split_threads(o, Filter::Needs)),
        (vec!["asks".into(), "broke".into()], 0)
    );
}

#[test]
fn default_children_take_the_remembered_else_the_first() {
    let sw = swarm(json!([orch(
        "p1",
        json!({"threads": [
            th("t1", "active", json!([{"id": "a", "status": "run"}, {"id": "b", "status": "run"}])),
            th("t2", "active", json!([{"id": "c", "status": "run"}])),
        ]})
    )]));
    let mut v = View {
        orch: Some("p1".into()),
        ..View::default()
    };
    default_children(&sw, &mut v);
    assert_eq!(
        (v.thread.as_deref(), v.agent.as_deref()),
        (Some("t1"), Some("a"))
    );
    v.remember_thread.insert("p1".into(), "t2".into());
    default_children(&sw, &mut v);
    assert_eq!(
        (v.thread.as_deref(), v.agent.as_deref()),
        (Some("t2"), Some("c"))
    );
    v.remember_thread.insert("p1".into(), "gone".into());
    v.remember_agent.insert("t1".into(), "b".into());
    default_children(&sw, &mut v);
    assert_eq!(
        (v.thread.as_deref(), v.agent.as_deref()),
        (Some("t1"), Some("b"))
    );
}

#[test]
fn the_path_to_an_agent_runs_from_the_threads_own() {
    let t: Thread = serde_json::from_value(th(
        "t",
        "active",
        json!([
            {"id": "a"}, {"id": "b", "parent": "a"}, {"id": "c", "parent": "b"},
        ]),
    ))
    .unwrap();
    assert_eq!(path_ids(Some(&t), Some("c")), ["a", "b", "c"]);
    assert!(path_ids(Some(&t), Some("x")).is_empty());
    assert!(path_ids(None, Some("c")).is_empty());
}
