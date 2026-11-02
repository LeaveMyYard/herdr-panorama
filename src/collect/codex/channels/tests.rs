use super::*;
use crate::codex::{Observation, Thread};
use serde_json::json;

fn observation() -> Observation {
    Observation {
        root: Thread {
            thread_id: "root".into(),
            ..Thread::default()
        },
        children: vec![Thread {
            thread_id: "child".into(),
            parent_thread_id: Some("root".into()),
            ..Thread::default()
        }],
        partial: false,
        observed_at: 10.0,
    }
}

fn write(dir: &Path, id: &str, name: &str) {
    fs::write(
        events_path(dir, id),
        format!("{}\n", json!({"v":1,"thread":"plan","name":name})),
    )
    .unwrap();
}

#[test]
fn only_fresh_complete_verified_descendants_authorize_channel_reads() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "root", "Root");
    write(dir.path(), "child", "Child");
    write(dir.path(), "foreign", "Foreign");
    let mut channels = Channels::new(dir.path().to_owned());
    let mut observation = observation();
    observation.children.push(Thread {
        thread_id: "foreign".into(),
        parent_thread_id: Some("another-root".into()),
        ..Thread::default()
    });
    let reports = channels.read("root", Some(&observation), 12.0);
    assert_eq!(
        reports
            .iter()
            .map(|r| r.source.as_str())
            .collect::<Vec<_>>(),
        ["root", "child"]
    );
    assert_eq!(reports[1].declarations[0].name.as_deref(), Some("Child"));
    observation.partial = true;
    assert_eq!(channels.read("root", Some(&observation), 12.0).len(), 1);
    observation.partial = false;
    assert_eq!(channels.read("root", Some(&observation), 30.0).len(), 1);
    assert_eq!(channels.read("root", None, 12.0).len(), 1);
    observation.root.thread_id = "wrong-root".into();
    assert_eq!(channels.read("root", Some(&observation), 12.0).len(), 1);
}

#[test]
fn unsafe_ids_oversized_files_and_symlinks_do_not_supply_reports() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "root", "Root");
    let mut channels = Channels::new(dir.path().to_owned());
    assert!(channels.read("../root", None, 12.0).is_empty());
    fs::write(
        events_path(dir.path(), "large"),
        vec![b'x'; MAX_FILE as usize + 1],
    )
    .unwrap();
    assert!(
        channels.read("large", None, 12.0)[0]
            .declarations
            .is_empty()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            events_path(dir.path(), "root"),
            events_path(dir.path(), "linked"),
        )
        .unwrap();
        assert!(
            channels.read("linked", None, 12.0)[0]
                .declarations
                .is_empty()
        );
    }
}

#[test]
fn cached_reports_are_not_returned_after_file_disappears_or_binding_changes() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "root", "Root");
    write(dir.path(), "child", "Child");
    let mut channels = Channels::new(dir.path().to_owned());
    assert_eq!(channels.read("root", Some(&observation()), 12.0).len(), 2);
    fs::remove_file(events_path(dir.path(), "child")).unwrap();
    assert!(
        channels.read("root", Some(&observation()), 12.0)[1]
            .declarations
            .is_empty()
    );
    let reports = channels.read("other", Some(&observation()), 12.0);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].source, "other");
    assert!(reports[0].declarations.is_empty());
}

#[test]
fn per_collect_channel_and_byte_budgets_bound_all_panes() {
    let dir = tempfile::tempdir().unwrap();
    let mut channels = Channels::new(dir.path().to_owned());
    for index in 0..32 {
        assert_eq!(channels.read(&format!("root-{index}"), None, 12.0).len(), 1);
    }
    assert!(channels.read("root-33", None, 12.0).is_empty());
    channels.begin_collect();
    assert_eq!(channels.read("root-33", None, 12.0).len(), 1);
    let record = format!(
        "{}\n",
        json!({"v":1,"thread":"plan","name":"x".repeat(100_000)})
    );
    for index in 0..6 {
        let id = format!("large-{index}");
        fs::write(events_path(dir.path(), &id), &record).unwrap();
        let reports = channels.read(&id, None, 12.0);
        assert_eq!(reports[0].declarations.is_empty(), index >= 5);
    }
}
