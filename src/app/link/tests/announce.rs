//! tests/test_panel_sync.py::test_the_core_side_publishes_an_announced_focus: a pane a panel
//! announces is what the home view publishes at once, before herdr says anything.

use super::link;
use herdr_agent_kit::herdr_api::fake::FakeHerdr;

#[tokio::test(flavor = "current_thread")]
async fn an_announced_focus_is_published_at_once() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut l = link(&fake, true, dir.path());
    assert_eq!(l.published.focused_pane, None);
    l.focusing("w1:p94");
    assert_eq!(l.published.focused_pane.as_deref(), Some("w1:p94"));
}
