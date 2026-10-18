use super::*;
use crate::collect::events::Channel;

#[test]
fn a_line_is_versioned_and_stamped_in_python_s_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("s.jsonl");
    emit(
        &path,
        vec![
            ("thread".into(), J::str("t")),
            ("phases".into(), J::Arr(vec![])),
        ],
        1_791_960_000.9,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "{\"v\": 1, \"ts\": \"2026-10-14T06:40:00Z\", \"thread\": \"t\", \"phases\": []}\n"
    );
    assert!(lock_path(&path).exists());
}

#[test]
fn the_file_is_compacted_past_its_limit_and_keeps_every_thread() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let big = "x".repeat(150);
    for i in 0..600_i32 {
        let rec = vec![
            ("thread".into(), J::str(if i % 2 == 0 { "a" } else { "b" })),
            ("name".into(), J::str(format!("{big} {i}"))),
            ("pct".into(), J::Int(i64::from(i % 100))),
        ];
        emit(&path, rec, 1_791_960_000.0 + f64::from(i)).unwrap();
    }
    assert!(fs::metadata(&path).unwrap().len() < 2 * COMPACT_AT);
    let mut ch = Channel::new(path.clone());
    ch.update();
    let got: Vec<_> = ch
        .threads
        .iter()
        .map(|d| (d.thread.as_str(), d.name.clone().unwrap(), d.pct))
        .collect();
    assert_eq!(
        got,
        [
            ("a", format!("{big} 598"), Some(98)),
            ("b", format!("{big} 599"), Some(99))
        ]
    );
    assert!(ch.threads.iter().all(|d| d.ts.is_some()));
}
