use super::*;

#[test]
fn explicit_reports_claim_children_once_and_descendants_follow() {
    let observation = observed();
    let mut channel = channel();
    channel.apply(
        json!({"v":1,"thread":"plan","name":"Synthetic plan",
        "agents":[{"match":"child","role":"reviewer"}],
        "phases":[{"name":"Review","state":"now"}],"status":"active"})
        .as_object()
        .unwrap(),
    );
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel);
    assert_eq!(pane.threads.len(), 2);
    assert_eq!(pane.threads[0].agents.len(), 1);
    let declared = &pane.threads[1];
    assert_eq!(declared.id, "x:codex:root:plan");
    assert_eq!(declared.agents.len(), 2);
    let child = declared.agents.iter().find(|a| a.id == "child").unwrap();
    assert_eq!(child.role, "reviewer");
    let grandchild = declared
        .agents
        .iter()
        .find(|a| a.id == "grandchild")
        .unwrap();
    assert_eq!(grandchild.parent.as_deref(), Some("child"));
    assert_eq!(pane.agents().filter(|a| a.id == "child").count(), 1);
    assert_eq!(pane.session_id.as_deref(), Some("root"));
}

#[test]
fn report_without_history_is_visible_and_does_not_infer_completion() {
    let mut channel = channel();
    channel.apply(
        json!({"v":1,"thread":"plan","name":"Synthetic plan"})
            .as_object()
            .unwrap(),
    );
    let mut pane = pane("root");
    apply(&mut pane, None, &channel);
    assert_eq!(pane.threads.len(), 1);
    assert_eq!(pane.threads[0].source, ThreadSource::Explicit);
    assert_eq!(pane.threads[0].status, ThreadStatus::from_code("unknown"));
}

#[test]
fn explicit_reports_cannot_claim_the_root_card() {
    let observation = observed();
    let mut channel = channel();
    channel.apply(
        json!({"v":1,"thread":"plan","status":"done", "ts":1,
        "agents":[{"match":"Synthetic root"}]})
        .as_object()
        .unwrap(),
    );
    let mut pane = pane("root");
    apply(&mut pane, Some(&observation), &channel);
    assert_eq!(pane.threads[0].agents[0].id, "main:codex:w1:p1");
    assert!(pane.threads[1].agents.is_empty());
    assert_eq!(
        pane.agents().filter(|a| a.id.starts_with("main:")).count(),
        1
    );
}

fn reports(source: &str, agents: serde_json::Value) -> channels::Reports {
    let mut channel = channel();
    channel.apply(
        json!({"v":1,"thread":"same-plan","agents":agents})
            .as_object()
            .unwrap(),
    );
    channels::Reports {
        source: source.into(),
        declarations: channel.threads,
    }
}

#[test]
fn sibling_declaration_ids_are_namespaced_and_child_claims_are_scoped() {
    let mut observation = observed();
    observation.children.push(thread("sibling", Some("root")));
    let reports = [
        reports(
            "child",
            json!([
                {"match":"sibling"}, {"match":"main:codex:w1:p1"}, {"match":"child"}
            ]),
        ),
        reports("sibling", json!([])),
    ];
    let mut pane = pane("root");
    enrich(&mut pane, Some(&observation), &reports, 40.0, 3600.0);
    assert_eq!(pane.threads.len(), 3);
    assert_eq!(pane.threads[0].agents.len(), 1);
    assert_eq!(pane.threads[1].id, "x:codex:child:same-plan");
    assert_eq!(pane.threads[2].id, "x:codex:sibling:same-plan");
    assert_eq!(
        pane.threads[1]
            .agents
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["child", "grandchild"]
    );
    assert_eq!(pane.threads[2].agents[0].id, "sibling");
    assert_eq!(pane.agents().count(), 4);
}

#[test]
fn root_declaration_claims_are_stronger_than_descendant_reports() {
    let reports = [
        reports("root", json!([{"match":"child"}])),
        reports("child", json!([])),
    ];
    let mut pane = pane("root");
    enrich(&mut pane, Some(&observed()), &reports, 40.0, 3600.0);
    assert_eq!(pane.threads[1].id, "x:codex:root:same-plan");
    assert_eq!(pane.threads[1].agents.len(), 2);
    assert!(pane.threads[2].agents.is_empty());
    assert_eq!(pane.agents().filter(|agent| agent.id == "child").count(), 1);
}
