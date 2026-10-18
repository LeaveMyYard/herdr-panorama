use super::*;
use crate::panel::act::fake::herdr;
use herdr_agent_kit::herdr_api::fake::Reply;
use serde_json::json;
use std::cell::RefCell;

/// Records the announcements; echoes them when `echo`.
#[derive(Default)]
struct Core {
    said: RefCell<Vec<String>>,
    echo: bool,
    up: bool,
}

impl Announce for Core {
    async fn focusing(&self, pane: &str) -> bool {
        self.said.borrow_mut().push(pane.to_owned());
        self.up
    }
    async fn echoed(&self, _: &str, _: Duration) -> bool {
        self.echo
    }
}

fn panels() -> HashMap<String, String> {
    [("w1:t4".to_owned(), "w1:p94".to_owned())].into()
}

#[tokio::test(flavor = "current_thread")]
async fn onto_the_target_tabs_panel_after_announcing_it() {
    let fake = herdr(&[("w1:p4", "w1:t4"), ("w1:p94", "w1:t4")]);
    fake.on("tab.focus", Reply::Result(json!({})));
    fake.on("pane.focus", Reply::Result(json!({})));
    let core = Core {
        echo: true,
        up: true,
        ..Core::default()
    };
    let done = switch(
        &fake.client(),
        "w1:p4",
        true,
        &panels(),
        Some("w1:p92"),
        &core,
    )
    .await;
    assert_eq!(
        done,
        Switched {
            err: None,
            took_back: false
        }
    );
    assert_eq!(*core.said.borrow(), ["w1:p94"]);
    assert_eq!(fake.params_of("tab.focus"), [json!({"tab_id": "w1:t4"})]);
    assert_eq!(fake.params_of("pane.focus"), [json!({"pane_id": "w1:p94"})]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_switch_takes_the_announcement_back() {
    let fake = herdr(&[("w1:p4", "w1:t4"), ("w1:p94", "w1:t4")]);
    fake.on("tab.focus", Reply::error("tab_not_found", "no tab"));
    let core = Core {
        echo: true,
        up: true,
        ..Core::default()
    };
    let done = switch(
        &fake.client(),
        "w1:p4",
        true,
        &panels(),
        Some("w1:p92"),
        &core,
    )
    .await;
    assert_eq!(done.err.as_deref(), Some("tab_not_found: no tab"));
    assert!(done.took_back);
    assert_eq!(*core.said.borrow(), ["w1:p94", "w1:p92"]);
}

#[tokio::test(flavor = "current_thread")]
async fn into_the_agent_without_keep_focus_or_without_a_panel_there() {
    let fake = herdr(&[("w1:p4", "w1:t4"), ("w1:p94", "w1:t4")]);
    fake.on("agent.focus", Reply::Result(json!({})));
    let core = Core::default();
    let done = switch(&fake.client(), "w1:p4", false, &panels(), None, &core).await;
    assert_eq!(done.err, None);
    assert_eq!(fake.params_of("agent.focus"), [json!({"target": "w1:p4"})]);
    // the tab's panel is not in the tab (it was closed): into the agent too
    let fake = herdr(&[("w1:p4", "w1:t4")]);
    fake.on("agent.focus", Reply::Result(json!({})));
    switch(&fake.client(), "w1:p4", true, &panels(), None, &core).await;
    assert_eq!(fake.params_of("agent.focus").len(), 1);
    assert!(core.said.borrow().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn a_pane_herdr_no_longer_has_is_gone() {
    let fake = herdr(&[]);
    let done = switch(
        &fake.client(),
        "w1:p4",
        true,
        &panels(),
        None,
        &Core::default(),
    )
    .await;
    assert_eq!(done.err.as_deref(), Some("w1:p4 is gone"));
}
