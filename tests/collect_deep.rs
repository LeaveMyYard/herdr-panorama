//! Subagents nested at any depth stay in their root's one tree, and a finished ancestor outside
//! the done window stays while an agent it spawned is shown (Python `test_deep.py`'s collector row,
//! over its own builder's files, captured by `tools/capture_deep.py`).

mod collect_support;

use collect_support::{STALE, Scenario};
use panorama_core::model::Status;
use std::path::Path;

#[test]
fn collector_nests_any_depth_in_one_tree_and_keeps_finished_ancestors() {
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
    assert_eq!(o.threads.len(), 1);
    let by = |id: &str| {
        o.agents()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("{id} in the tree"))
    };
    // parents before children: the chain in order, its top a root child
    let order: Vec<&str> = o.agents().map(|a| a.id.as_str()).collect();
    let at = |id: &str| order.iter().position(|x| *x == id).unwrap();
    assert!(chain.windows(2).all(|w| at(&w[0]) < at(&w[1])), "{order:?}");
    assert_eq!(by("a1").parent, None);
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
    // the depth is the chain's
    let depths: Vec<i64> = chain.iter().map(|id| by(id).depth).collect();
    assert_eq!(depths, (1..=10).collect::<Vec<_>>());
    // a1 finished before the window, yet stays, and is not counted hidden
    let a1_end = by("a1").ended.or(by("a1").last_at).unwrap();
    assert!(a1_end < sc.now() - window, "{a1_end}");
    assert_eq!(o.hidden_finished, 0);
}
