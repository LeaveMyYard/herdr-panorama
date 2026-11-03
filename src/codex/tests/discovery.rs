use super::{calls, fixture};
use crate::codex::{discovery::Graph, history::fetch_cancellable};
use std::{collections::HashSet, sync::atomic::AtomicBool};

pub(super) fn setup(mode: &str) -> (tempfile::TempDir, crate::codex::Options) {
    let (dir, options) = fixture(mode);
    std::fs::write(&options.executable, include_str!("discovery_fixture.py")).unwrap();
    (dir, options)
}
async fn refresh(
    options: &crate::codex::Options,
    graph: &mut Graph,
    root: &str,
    unsupported: &mut HashSet<String>,
) -> crate::codex::Observation {
    fetch_cancellable(options, root, unsupported, &AtomicBool::new(false), graph)
        .await
        .unwrap()
}
#[tokio::test]
async fn progressive_pages_find_six_old_siblings_and_retain_them_after_references_age_out() {
    let (dir, options) = setup("progress");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let first = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
    assert!(first.children.is_empty() && first.partial);
    let mut found = first;
    for _ in 0..7 {
        found = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
    }
    assert_eq!(found.children.len(), 6);
    assert!(
        found
            .children
            .iter()
            .all(|t| t.parent_thread_id.as_deref() == Some("root-a"))
    );
    assert!(
        found.partial,
        "display truncation must not authorize descendant reports"
    );
    std::fs::write(dir.path().join("mode"), "aged").unwrap();
    let retained = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
    assert_eq!(retained.children.len(), 6);
    let before = calls(dir.path()).len();
    std::fs::write(dir.path().join("mode"), "changed-parent").unwrap();
    let changed = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
    assert_eq!(changed.children.len(), 5);
    assert!(changed.partial);
    assert!(
        calls(dir.path())[before..]
            .iter()
            .any(|c| c["method"] == "thread/read" && c["params"]["threadId"] == "child-0")
    );
    let other = refresh(&options, &mut graph, "root-b", &mut unsupported).await;
    assert!(
        other.children.is_empty(),
        "root switch must clear ancestry candidates"
    );
}
#[tokio::test]
async fn full_turn_and_legacy_backfill_cross_display_item_and_turn_limits() {
    for mode in ["full-progress", "legacy-progress"] {
        let (_dir, options) = setup(mode);
        let mut graph = Graph::new("root-a");
        let mut unsupported = HashSet::new();
        let first = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        assert!(first.children.is_empty() && first.partial, "{mode}");
        let second = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        assert_eq!(second.children.len(), 6, "{mode}");
        assert!(second.partial, "{mode}");
    }
}
#[tokio::test]
async fn cached_candidates_never_authorize_wrong_roots_cycles_or_fabricated_runtime() {
    let (_dir, options) = setup("scope");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
    assert_eq!(
        value
            .children
            .iter()
            .map(|t| t.thread_id.as_str())
            .collect::<Vec<_>>(),
        ["child-0", "grand"]
    );
    assert!(value.partial);
    assert!(value.children.iter().all(|t| {
        t.turns
            .iter()
            .all(|t| t.status == crate::codex::TurnStatus::Completed)
    }));
}
#[tokio::test]
async fn hung_backfill_uses_whole_refresh_deadline_and_reaps_owned_observer() {
    for mode in ["backfill-hang", "full-hang"] {
        let (_dir, mut options) = setup(mode);
        options.timeout = std::time::Duration::from_millis(400);
        let mut graph = Graph::new("root-a");
        let start = std::time::Instant::now();
        assert!(
            fetch_cancellable(
                &options,
                "root-a",
                &mut HashSet::new(),
                &AtomicBool::new(false),
                &mut graph
            )
            .await
            .is_err()
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
        #[cfg(target_os = "linux")]
        assert!(
            !std::path::Path::new(&format!(
                "/proc/{}",
                std::fs::read_to_string(_dir.path().join("pid")).unwrap()
            ))
            .exists()
        );
    }
}

#[tokio::test]
async fn foreign_candidate_prefix_cannot_starve_a_valid_later_sibling() {
    let (_dir, options) = setup("fair");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let mut found = false;
    for _ in 0..16 {
        let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        if value.children.iter().any(|t| t.thread_id == "child-0") {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "bounded refreshes must reach the valid child after invalid candidates"
    );
    for _ in 0..3 {
        assert!(
            refresh(&options, &mut graph, "root-a", &mut unsupported)
                .await
                .children
                .iter()
                .any(|t| t.thread_id == "child-0")
        );
    }
}
#[tokio::test]
async fn soft_backfill_budget_never_cancels_a_partial_frame_before_child_reads() {
    let (_dir, options) = setup("split-backfill");
    let mut graph = Graph::new("root-a");
    let value = refresh(&options, &mut graph, "root-a", &mut HashSet::new()).await;
    assert!(value.children.iter().any(|t| t.thread_id == "child-0"));
    assert!(value.partial);
}

#[tokio::test]
async fn cancellation_during_backfill_reaps_the_observer_without_reusing_its_stream() {
    let (dir, mut options) = setup("backfill-hang");
    options.timeout = std::time::Duration::from_secs(4);
    let stop = AtomicBool::new(false);
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let query = fetch_cancellable(&options, "root-a", &mut unsupported, &stop, &mut graph);
    let cancel = async {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if calls(dir.path())
                .iter()
                .any(|c| c["params"]["cursor"] == "128")
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        std::time::Instant::now()
    };
    let (result, cancelled) = tokio::join!(query, cancel);
    assert!(result.is_err());
    assert!(cancelled.elapsed() < std::time::Duration::from_millis(300));
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!(
            "/proc/{}",
            std::fs::read_to_string(dir.path().join("pid")).unwrap()
        ))
        .exists()
    );
}

#[tokio::test]
async fn oversized_valid_graphs_keep_the_eight_read_cap_without_starving_new_siblings() {
    let (dir, options) = setup("wide");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let mut seen = HashSet::new();
    let mut recent = HashSet::new();
    for round in 0..16 {
        let before = calls(dir.path()).len();
        let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        assert!(value.partial && value.children.len() <= 7);
        assert!(
            calls(dir.path())[before..]
                .iter()
                .filter(|c| c["method"] == "thread/read")
                .count()
                <= 8
        );
        if round >= 12 {
            recent.extend(value.children.iter().map(|t| t.thread_id.clone()));
        }
        seen.extend(value.children.into_iter().map(|t| t.thread_id));
    }
    assert_eq!(seen.len(), 12);
    assert_eq!(
        recent.len(),
        12,
        "previously verified oversized siblings also rotate"
    );
}

#[tokio::test]
async fn within_eight_valid_nodes_cross_depth_work_is_not_starved_by_foreign_references() {
    let (dir, options) = setup("breadth");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let mut found = false;
    for _ in 0..10 {
        let before = calls(dir.path()).len();
        let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        assert!(
            calls(dir.path())[before..]
                .iter()
                .filter(|c| c["method"] == "thread/read")
                .count()
                <= 8
        );
        if value.children.iter().any(|t| t.thread_id == "grand") {
            assert_eq!(value.children.len(), 7);
            found = true;
        }
        assert!(
            value.partial,
            "foreign reference cannot authorize a complete graph"
        );
    }
    assert!(
        found,
        "root+6direct+grandchild must fit the unchanged eight-thread cap"
    );
    assert!(
        calls(dir.path())
            .iter()
            .any(|c| c["method"] == "thread/read" && c["params"]["threadId"] == "grand")
    );
}
#[tokio::test]
async fn rejected_parent_metadata_cannot_evict_a_valid_childs_old_backfill() {
    let (dir, options) = setup("scan-eviction");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let mut found = false;
    for _ in 0..10 {
        let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        if value.children.iter().any(|t| t.thread_id == "grand") {
            found = true;
        }
    }
    assert!(
        found,
        "eight foreign IDs must not reset the only valid child's scan before item800"
    );
    let calls = calls(dir.path());
    assert!(calls.iter().any(|c| c["method"] == "thread/items/list"
        && c["params"]["threadId"] == "a-child"
        && c["params"]["cursor"] == "768"));
    assert!(!calls.iter().any(|c| {
        matches!(
            c["method"].as_str(),
            Some("thread/items/list" | "thread/turns/list")
        ) && c["params"]["threadId"]
            .as_str()
            .is_some_and(|id| id.starts_with("bad-"))
    }));
}

#[tokio::test]
async fn wrong_scope_reference_cannot_refresh_or_erase_the_correct_parent_edge() {
    let (dir, options) = setup("overlap");
    let mut graph = Graph::new("root-a");
    let mut unsupported = HashSet::new();
    let mut seen = HashSet::new();
    for _ in 0..10 {
        let before = calls(dir.path()).len();
        let value = refresh(&options, &mut graph, "root-a", &mut unsupported).await;
        assert!(
            calls(dir.path())[before..]
                .iter()
                .filter(|c| c["method"] == "thread/read")
                .count()
                <= 8
        );
        for thread in value.children {
            if thread.thread_id == "a-grand" {
                assert_eq!(thread.parent_thread_id.as_deref(), Some("b-child"));
            }
            if thread.thread_id == "g-grand" {
                assert_eq!(thread.parent_thread_id.as_deref(), Some("c-child"));
            }
            seen.insert(thread.thread_id);
        }
    }
    assert!(
        seen.contains("a-grand") && seen.contains("g-grand"),
        "both valid grandchildren must appear despite the wrong root-scoped reference: {seen:?}"
    );
    assert_eq!(seen.len(), 7);
}
