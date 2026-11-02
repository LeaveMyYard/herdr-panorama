use super::{calls, fixture};
use crate::codex::{TurnStatus, history::fetch, protocol::Error};
use std::time::{Duration, Instant};

#[tokio::test]
async fn paginated_history_verifies_exact_ids_parentage_and_turn_scoped_tools() {
    let (dir, options) = fixture("normal");
    let data = fetch(&options, "root-a").await.unwrap();
    assert_eq!(data.root.thread_id, "root-a");
    assert_eq!(data.root.session_tree_id, "shared-tree");
    assert_eq!(data.root.model.as_deref(), Some("synthetic-model"));
    assert_eq!(data.root.turns[0].status, TurnStatus::InProgress);
    assert_eq!(data.root.turns[1].status, TurnStatus::Completed);
    assert_eq!(data.root.turns[0].prompt, "Synthetic prompt");
    assert_eq!(data.root.turns[1].summary, "Earlier answer");
    assert_eq!(data.root.turns[0].tool_count, 2); // duplicate c2 is ignored
    assert_eq!(
        data.root.latest_tool.as_ref().unwrap().started_at,
        Some(151.0)
    );
    assert_eq!(
        data.root.turns[1].latest_tool.as_ref().unwrap().turn_id,
        "t1"
    );
    assert_eq!(
        data.children
            .iter()
            .map(|t| t.thread_id.as_str())
            .collect::<Vec<_>>(),
        ["child-a", "grand-a"]
    );
    assert_eq!(
        data.children[1].parent_thread_id.as_deref(),
        Some("child-a")
    );
    assert!(data.partial); // foreign reference has unverified parentage
    assert!(!format!("{data:?}").contains("SECRET"));
    let calls = calls(dir.path());
    assert!(calls.iter().all(|c| matches!(
        c["method"].as_str(),
        Some(
            "initialize"
                | "initialized"
                | "thread/read"
                | "thread/turns/list"
                | "thread/items/list"
        )
    )));
    assert!(calls.iter().any(|c| c["params"]["cursor"] == "page-2"));
    assert!(
        calls
            .iter()
            .filter(|c| c["method"] == "thread/read")
            .all(|c| c["params"]["includeTurns"] == false)
    );
}
#[tokio::test]
async fn legacy_fallback_is_only_for_unsupported_legacy_history() {
    let (dir, options) = fixture("legacy");
    let data = fetch(&options, "root-a").await.unwrap();
    assert_eq!(data.root.turns[0].prompt, "Legacy prompt");
    assert_eq!(data.root.turns[0].summary, "Legacy answer");
    assert_eq!(data.root.turns[0].tool_count, 1);
    assert!(
        calls(dir.path())
            .iter()
            .any(|c| c["params"]["includeTurns"] == true)
    );
    let (dir, options) = fixture("unsupported");
    assert_eq!(fetch(&options, "root-a").await, Err(Error::Unsupported));
    assert!(
        !calls(dir.path())
            .iter()
            .any(|c| c["params"]["includeTurns"] == true)
    );
}
#[tokio::test]
async fn malformed_oversized_wrong_identity_and_timeout_fail_closed() {
    for (mode, expected) in [
        ("malformed", Error::Invalid),
        ("huge", Error::Limit),
        ("mismatch", Error::Invalid),
        ("hang", Error::Unavailable),
    ] {
        let (dir, mut options) = fixture(mode);
        options.timeout = if mode == "hang" {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(2)
        };
        let start = Instant::now();
        assert_eq!(fetch(&options, "root-a").await, Err(expected), "{mode}");
        assert!(start.elapsed() < Duration::from_secs(3));
        #[cfg(target_os = "linux")]
        {
            let pid = std::fs::read_to_string(dir.path().join("pid")).unwrap();
            assert!(
                !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                "observer not reaped: {mode}"
            );
        }
    }
}
#[tokio::test]
async fn failed_items_and_repeated_cursor_retain_metadata_as_partial() {
    for mode in ["items-fail", "repeat-cursor"] {
        let (_dir, options) = fixture(mode);
        let data = fetch(&options, "root-a").await.unwrap();
        assert!(data.partial);
        assert_eq!(data.root.thread_id, "root-a");
    }
}
#[tokio::test]
async fn same_cwd_or_tree_never_changes_the_requested_thread() {
    let (_dir, options) = fixture("normal");
    let data = fetch(&options, "root-b").await.unwrap();
    assert_eq!(data.root.thread_id, "root-b");
    assert_eq!(data.root.session_tree_id, "shared-tree");
    assert!(data.children.is_empty());
}

#[tokio::test]
async fn installed_version_items_gap_falls_back_to_full_turns_and_remembers_capability() {
    for mode in ["no-items", "no-items-legacy"] {
        let (dir, options) = fixture(mode);
        let mut paging = std::collections::HashSet::new();
        for _ in 0..2 {
            let data = crate::codex::history::fetch_with(&options, "root-a", &mut paging)
                .await
                .unwrap();
            assert_eq!(data.root.turns[0].prompt, "Full turn fallback");
            assert_eq!(data.root.turns[0].summary, "Full answer");
            assert_eq!(data.children[0].thread_id, "child-a");
            assert_eq!(data.root.turns[0].tool_count, 2);
        }
        assert_eq!(paging.len(), 1);
        assert_eq!(
            calls(dir.path())
                .iter()
                .filter(|c| c["method"] == "thread/items/list")
                .count(),
            1
        );
        assert!(
            !calls(dir.path())
                .iter()
                .any(|c| c["params"]["includeTurns"] == true)
        );
    }
}

#[tokio::test]
async fn legacy_capability_does_not_disable_paginated_item_reads() {
    let (dir, options) = fixture("mixed");
    let mut unsupported = std::collections::HashSet::new();
    crate::codex::history::fetch_with(&options, "root-a", &mut unsupported)
        .await
        .unwrap();
    assert!(unsupported.contains("legacy"));
    let data = crate::codex::history::fetch_with(&options, "root-b", &mut unsupported)
        .await
        .unwrap();
    assert_eq!(data.root.turns[0].prompt, "Synthetic prompt");
    assert!(
        calls(dir.path())
            .iter()
            .any(|c| c["method"] == "thread/items/list" && c["params"]["threadId"] == "root-b")
    );
}

#[tokio::test]
async fn protocol_rejects_mutating_methods_before_writing_them() {
    let (dir, options) = fixture("normal");
    let mut client = crate::codex::protocol::Client::spawn(&options).unwrap();
    for method in [
        "thread/start",
        "thread/resume",
        "turn/start",
        "account/login/start",
    ] {
        assert_eq!(
            client.call(method, serde_json::json!({})).await,
            Err(Error::Invalid)
        );
    }
    client.child.start_kill().unwrap();
    client.child.wait().await.unwrap();
    assert!(calls(dir.path()).is_empty());
}
