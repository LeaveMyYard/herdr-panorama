//! Subagents nested at any depth stay in one thread, and a finished ancestor outside the done
//! window stays while an agent it spawned is shown (Python `test_deep.py`'s collector row,
//! over its own builder's files, captured by `tools/capture_deep.py`).

mod collect_support;

use collect_support::{STALE, Scenario};
use panorama_core::model::Status;
use std::path::Path;

#[test]
fn collector_nests_any_depth_in_one_thread_and_keeps_finished_ancestors() {
    let sc = Scenario::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/collect/deep.json"),
    );
    let window = sc.input["done_window"].as_f64().unwrap();
    let sw = sc
        .collector()
        .collect(Some(sc.snapshot()), None, sc.now(), window, STALE);
    let o = &sw.orchestrators[0];
    let chain: Vec<String> = ["a1".to_owned()]
        .into_iter()
        .chain((2..=10).map(|i| format!("d{i}")))
        .collect();
    let t = o
        .threads
        .iter()
        .find(|t| t.agents.iter().any(|a| a.id == "d10"))
        .expect("d10's thread");
    let by = |id: &str| {
        t.agents
            .iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("{id} in its thread"))
    };
    for id in &chain {
        by(id); // one thread holds the whole chain
    }
    assert_eq!(
        (by("a1").status.clone(), by("d10").status.clone()),
        (Status::Done, Status::Running)
    );
    let parents: Vec<Option<&str>> = chain[1..]
        .iter()
        .map(|id| by(id).parent.as_deref())
        .collect();
    let want: Vec<Option<&str>> = chain[..chain.len() - 1]
        .iter()
        .map(|id| Some(id.as_str()))
        .collect();
    assert_eq!(parents, want);
    assert_eq!(by("d10").depth, 10);
    // a nested agent takes its parent's (last) phase
    let top = by("a1").phases.last().copied().expect("a1 has a phase");
    assert!(
        chain.iter().all(|id| by(id).phases == [top]),
        "{:?}",
        chain.iter().map(|id| &by(id).phases).collect::<Vec<_>>()
    );
}
