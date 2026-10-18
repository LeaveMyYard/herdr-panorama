//! A scripted herdr for the effects' tests: the kit's `FakeHerdr` with a snapshot.

use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

/// A fake herdr whose snapshot has these `(pane, tab)`s.
pub fn herdr(panes: &[(&str, &str)]) -> FakeHerdr {
    let fake = FakeHerdr::start();
    let panes: Vec<_> = panes
        .iter()
        .map(|(p, t)| json!({"pane_id": p, "tab_id": t}))
        .collect();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({ "snapshot": { "panes": panes } })),
    );
    fake
}
