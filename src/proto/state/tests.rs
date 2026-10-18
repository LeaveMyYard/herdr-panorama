use super::*;
use serde_json::json;

fn captured_swarm() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    v["swarm"].clone()
}

fn read(v: Value) -> Result<State, serde_json::Error> {
    State::read(v.as_object().unwrap())
}

#[test]
fn a_full_snapshot_reads() {
    let st = read(json!({
        "protocol": "1.3", "swarm": captured_swarm(),
        "unseen": {"b3": "fail", "c1": "wait", "a5": "done"},
        "focused_pane": "w1:p3", "visible_tabs": ["w1:t3"],
        "panels": {"w1:t3": "w1:p30"}, "filter": "needs",
    }))
    .unwrap();
    assert_eq!(st.swarm.orchestrators.len(), 5);
    assert_eq!(st.unseen.get("b3"), Some(&Unseen::Fail));
    assert_eq!(st.focused_pane.as_deref(), Some("w1:p3"));
    assert_eq!(st.visible_tabs, Some(vec!["w1:t3".to_owned()]));
    assert_eq!(st.panels.get("w1:t3").map(String::as_str), Some("w1:p30"));
    assert_eq!(st.filter, Some(Filter::Needs));
}

#[test]
fn fields_of_the_wrong_shape_read_as_absent() {
    // Python: unseen not a dict -> {}, panels keeps str -> str, visible_tabs not a list ->
    // unknown, focused_pane not a str -> none, an unknown filter is ignored
    let st = read(json!({
        "unseen": ["x"], "panels": {"t1": "p1", "t2": 5}, "visible_tabs": "w1:t1",
        "focused_pane": 3, "filter": "everything",
    }))
    .unwrap();
    assert!(st.unseen.is_empty());
    assert_eq!(st.panels.len(), 1);
    assert_eq!(st.visible_tabs, None);
    assert_eq!(st.focused_pane, None);
    assert_eq!(st.filter, None);
    assert_eq!(st.swarm, Swarm::default()); // no swarm: an empty one
    let st = read(json!({"unseen": {"a": "fail", "b": "new-kind"}, "visible_tabs": null})).unwrap();
    assert_eq!(st.unseen.len(), 1);
    assert_eq!(st.visible_tabs, None);
}

#[test]
fn a_swarm_that_does_not_read_rejects_the_state() {
    let mut swarm = captured_swarm();
    swarm["orchestrators"][0]["threads"][0]["agents"][0]["id"] = Value::Null;
    assert!(read(json!({"swarm": swarm})).is_err());
}

#[test]
fn every_string_is_cleaned_once_as_it_is_read() {
    let mut swarm = captured_swarm();
    swarm["orchestrators"][0]["tab_label"] = json!("evil\u{1b}]0;title\u{7}\ttab");
    swarm["orchestrators"][0]["threads"][0]["agents"][0]["id"] = json!("a\u{1b}1");
    swarm["errors"] = json!(["bad\nline"]);
    let st = read(json!({"swarm": swarm, "unseen": {"a\u{1b}1": "fail"}})).unwrap();
    let o = &st.swarm.orchestrators[0];
    assert_eq!(o.tab_label, "evil]0;title tab");
    assert_eq!(o.threads[0].agents[0].id, "a1");
    assert_eq!(st.swarm.errors, ["bad line"]);
    // the marks are cleaned the same way, so they still name the agent
    assert!(st.unseen.contains_key("a1"));
}
