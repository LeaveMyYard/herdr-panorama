//! The pane herdr says was focused when it invoked the plugin.

use super::invocation_pane;

#[test]
fn the_invocation_pane_is_the_contexts_focused_pane() {
    assert_eq!(
        invocation_pane(Some(r#"{"focused_pane_id": "w1:p2"}"#)).as_deref(),
        Some("w1:p2")
    );
    for none in [
        None,
        Some(""),
        Some("not json"),
        Some("[]"),
        Some(r#"{"focused_pane_id": ""}"#),
        Some(r#"{"focused_pane_id": 3}"#),
    ] {
        assert_eq!(invocation_pane(none), None, "{none:?}");
    }
}
