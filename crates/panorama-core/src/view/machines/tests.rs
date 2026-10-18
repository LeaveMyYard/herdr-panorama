use super::*;

#[test]
fn machine_groups_start_once_per_machine_id() {
    let o = |id: Option<&str>, label: Option<&str>| -> Orchestrator {
        serde_json::from_value(
            serde_json::json!({"pane_id": "p", "tab_id": null, "tab_label": "x",
            "session_id": null, "machine_id": id, "machine": label}),
        )
        .unwrap()
    };
    let (local, a1, a2, b) = (
        o(None, None),
        o(Some("m1"), Some("box")),
        o(Some("m1"), Some("box")),
        o(Some("m2"), Some("box")),
    );
    let mut prev = None;
    let starts: Vec<Option<&str>> = [&local, &a1, &a2, &b]
        .into_iter()
        .map(|o| group_start(o, &mut prev))
        .collect();
    // two machines may share a label: each still gets its divider
    assert_eq!(starts, [None, Some("box"), None, Some("box")]);
    let mut cv = Screen::new(12, 1);
    divider(&mut cv, 0, 12, "box");
    assert_eq!(cv.text(), "─ box ──────");
}
