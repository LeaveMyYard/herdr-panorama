use super::*;
use serde_json::json;
use std::io::Write;

fn rec(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn later_lines_update_earlier_ones_field_by_field() {
    let mut ch = Channel::new(PathBuf::from("/nonexistent"));
    ch.apply(&rec(
        json!({"v": 1, "thread": "t", "name": "work", "pct": 140.7, "status": "active",
                         "phases": [{"name": "a"}, {"x": 1}, "s"], "agents": [{"match": "ab*"}]}),
    ));
    ch.apply(&rec(
        json!({"v": 1.0, "thread": "t", "status": "bogus", "pct": true, "flavour": "pr"}),
    ));
    ch.apply(&rec(json!({"v": 2, "thread": "t", "name": "ignored"})));
    ch.apply(&rec(json!({"v": 1, "thread": ""})));
    ch.apply(&rec(json!({"v": true, "thread": "u", "pct": -3})));
    assert_eq!(ch.threads.len(), 2);
    let d = &ch.threads[0];
    assert_eq!(d.name.as_deref(), Some("work"));
    assert_eq!(d.flavour.as_deref(), Some("pr"));
    assert_eq!(d.status.as_deref(), Some("active"));
    assert_eq!(d.pct, Some(100));
    assert_eq!(d.phases.len(), 1);
    assert_eq!(d.agents.len(), 1);
    assert_eq!(ch.threads[1].pct, Some(0));
}

#[test]
fn the_file_is_followed_and_a_rewrite_starts_over() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    std::fs::write(&path, "{\"v\": 1, \"thread\": \"a\"}\n").unwrap();
    let mut ch = Channel::new(path.clone());
    ch.update();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{{\"v\": 1, \"thread\": \"b\"}}").unwrap();
    ch.update();
    assert_eq!(
        ch.threads
            .iter()
            .map(|d| d.thread.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    std::fs::write(&path, "{\"v\": 1, \"thread\": \"c\"}\n").unwrap(); // compacted: shorter
    ch.update();
    assert_eq!(
        ch.threads
            .iter()
            .map(|d| d.thread.as_str())
            .collect::<Vec<_>>(),
        ["c"]
    );
}

#[test]
fn patterns_claim_by_id_description_or_a_long_enough_prefix() {
    assert!(matches("a1b2", "a1b2", "x"));
    assert!(matches("review PR 3", "z", "review PR 3"));
    assert!(matches("revi*", "z", "review PR 3"));
    assert!(!matches("rev*", "z", "review PR 3"));
    assert!(!matches("*", "z", "anything"));
}

#[test]
fn phase_times_are_iso_or_hh_mm_on_the_reference_day() {
    let iso = phase_time(Some(&json!("2026-10-16T18:41:47Z")), None, 0.0).unwrap();
    assert_eq!(iso, 1_792_176_107.0);
    let base = 1_792_176_107.0;
    let t = phase_time(Some(&json!("09:30")), Some(base), 0.0).unwrap();
    let local = Local.timestamp_opt(t as i64, 0).unwrap();
    assert_eq!(local.format("%H:%M").to_string(), "09:30");
    let day = Local.timestamp_opt(base as i64, 0).unwrap().date_naive();
    assert_eq!(local.date_naive(), day);
    assert_eq!(phase_time(Some(&json!("9:x")), Some(base), 0.0), None);
    assert_eq!(phase_time(Some(&json!("")), Some(base), 0.0), None);
    assert_eq!(phase_time(Some(&json!(5)), Some(base), 0.0), None);
}
