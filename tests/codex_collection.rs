mod codex_support;

use codex_support::{Fixture, collect, ready, snapshot};
use panorama_core::model::{HerdrStatus, Status};
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn mixed_collection_enriches_exact_ids_and_preserves_panes_and_hierarchy() {
    let fixture = Fixture::new("paginated");
    let mut collector = fixture.collector(true);
    collector.self_pane = Some("panel".into());
    let swarm = ready(&mut collector, &snapshot());
    assert_eq!(swarm.orchestrators.len(), 4);
    assert_eq!(swarm.docked.as_deref(), Some("codex-a"));
    let find = |pane: &str| {
        swarm
            .orchestrators
            .iter()
            .find(|o| o.pane_id == pane)
            .unwrap()
    };
    assert_eq!(find("claude").harness, "claude");
    let a = find("codex-a");
    assert_eq!(a.harness, "codex");
    assert_eq!(a.session_id.as_deref(), Some("root-a"));
    assert_eq!(a.threads[0].id, "codex:root-a");
    assert_eq!(a.herdr_status, HerdrStatus::Working);
    assert_eq!(a.threads[0].agents[0].status, Status::Running);
    assert!(a.agents().any(|agent| agent.id == "child-a"));
    let grandchild = a.agents().find(|agent| agent.id == "grand-a").unwrap();
    assert_eq!(grandchild.parent.as_deref(), Some("child-a"));
    assert_eq!(grandchild.depth, 2);
    assert_eq!(grandchild.status, Status::from_code("unknown"));
    assert!(!a.agents().any(|agent| agent.id == "foreign"));
    let b = find("codex-b");
    assert_eq!(b.threads[0].id, "codex:root-b");
    assert_eq!(b.herdr_status, HerdrStatus::Blocked);
    assert!(!b.agents().any(|agent| agent.id == "child-a"));
    assert!(find("unbound").threads.is_empty());
    let wire = serde_json::to_value(&swarm).unwrap();
    let decoded: panorama_core::model::Swarm = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(decoded.orchestrators[1].harness, "codex");
    assert!(!wire.to_string().contains("SECRET NEVER RETURNED"));
    assert!(fixture.calls().iter().all(|call| matches!(
        call["method"].as_str(),
        Some(
            "initialize"
                | "initialized"
                | "thread/read"
                | "thread/turns/list"
                | "thread/items/list"
        )
    )));
}

#[test]
fn unresponsive_or_missing_codex_keeps_mixed_panes_and_returns_immediately() {
    let fixture = Fixture::new("hang");
    let mut collector = fixture.collector(true);
    let start = Instant::now();
    let swarm = collect(&mut collector, &snapshot());
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(swarm.orchestrators.len(), 4);
    assert!(swarm.orchestrators.iter().all(|o| o.threads.is_empty()));
    drop(collector);
    let mut options = fixture.options();
    options.executable = fixture.root().join("missing-codex");
    let mut collector = fixture.collector(false).with_codex(options);
    let swarm = ready(&mut collector, &snapshot());
    assert_eq!(swarm.orchestrators.len(), 4);
    assert!(swarm.orchestrators.iter().all(|o| o.threads.is_empty()));
}

#[test]
fn changed_binding_never_uses_cached_same_cwd_or_shared_group_history() {
    let fixture = Fixture::new("paginated");
    let mut collector = fixture.collector(true);
    let mut snapshot = snapshot();
    let original = ready(&mut collector, &snapshot);
    assert_eq!(original.orchestrators[1].threads[0].id, "codex:root-a");
    snapshot["agents"][1]["agent_session"]["value"] = json!("replacement");
    let immediate = collect(&mut collector, &snapshot);
    let replaced = &immediate.orchestrators[1];
    assert_eq!(replaced.session_id.as_deref(), Some("replacement"));
    assert!(replaced.threads.is_empty() || replaced.threads[0].id == "codex:replacement");
    let updated = ready(&mut collector, &snapshot);
    assert_eq!(updated.orchestrators[1].threads[0].id, "codex:replacement");
    assert!(
        !updated.orchestrators[1]
            .agents()
            .any(|agent| agent.id == "child-a")
    );
}

#[test]
fn mirror_collectors_leave_codex_visible_without_starting_local_history() {
    use panorama::collect::mirror::MirrorCollector;
    use panorama::machines::Collect;
    let fixture = Fixture::new("paginated");
    let mut mirror = MirrorCollector {
        collector: fixture.collector(false),
        windows: Box::new(|| (3600.0, 43200.0)),
        clock: Box::new(panorama::snapshot::now),
    };
    let snapshot = serde_json::from_value(snapshot()).unwrap();
    let swarm = mirror.collect(&snapshot);
    assert_eq!(swarm.orchestrators.len(), 4);
    assert!(swarm.orchestrators.iter().all(|o| o.threads.is_empty()));
    assert!(fixture.calls().is_empty());
}
