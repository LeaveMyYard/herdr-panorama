use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn the_popup_opens_in_the_directory_with_the_plugins_root() {
    let fake = FakeHerdr::start();
    fake.on("plugin.pane.open", Reply::Result(json!({})));
    let dir = tempfile::tempdir().unwrap();
    let mark = marker(dir.path(), "k");
    let out = open(
        &fake.client(),
        "leavemyyard.herdr-panorama",
        Some(Path::new("/opt/p")),
        "/work",
        &mark,
    )
    .await;
    assert_eq!(out, None);
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(p["entrypoint"], "new-agent");
    assert_eq!(p["placement"], "popup");
    assert_eq!(p["env"]["PANORAMA_NEW_AGENT_CWD"], "/work");
}

#[tokio::test(flavor = "current_thread")]
async fn no_reply_waits_for_the_popups_own_mark() {
    let dir = tempfile::tempdir().unwrap();
    let mark = marker(dir.path(), "k");
    let fake = FakeHerdr::start();
    fake.on("plugin.pane.open", Reply::Close);
    // nothing started within POPUP_START (a real 3 s wait): say so
    let out = open(&fake.client(), "p", None, "/w", &mark).await;
    assert_eq!(out.as_deref(), Some(DID_NOT_OPEN));
    // a mark from now: it opened
    std::fs::write(&mark, format!("{}", now() + 60.0)).unwrap();
    assert_eq!(open(&fake.client(), "p", None, "/w", &mark).await, None);
    // refused: herdr's words
    let fake = FakeHerdr::start();
    fake.on("plugin.pane.open", Reply::error("plugin_not_found", "nope"));
    assert_eq!(
        open(&fake.client(), "p", None, "/w", &mark)
            .await
            .as_deref(),
        Some("plugin_not_found: nope")
    );
}
