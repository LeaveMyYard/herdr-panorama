use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};

fn reply(ready: bool, pane: &str) -> serde_json::Value {
    json!({"agent":{"name":"synthetic","pane_id":pane,"agent":"codex",
        "agent_status":if ready {"idle"} else {"unknown"},
        "interactive_ready":ready,"launch_pending":!ready}})
}

#[tokio::test(flavor = "current_thread")]
async fn codex_start_passes_no_daemon_and_waits_for_readiness() {
    let fake = FakeHerdr::start();
    fake.on("agent.start", Reply::Result(reply(false, "w1:p1")));
    fake.on("agent.get", Reply::Result(reply(true, "w1:p1")));
    let herdr = fake.client();
    start(&herdr, "synthetic", "w1:p1", 2000).await.unwrap();
    assert_eq!(
        fake.params_of("agent.start"),
        [json!({
            "name":"synthetic","kind":"codex","pane_id":"w1:p1",
            "timeout_ms":2000,"args":["--no-daemon"]
        })]
    );
    assert!(!fake.params_of("agent.get").is_empty());
    assert!(fake.params_of("pane.send_text").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn an_unready_or_replaced_pane_is_not_a_successful_codex_launch() {
    let fake = FakeHerdr::start();
    let herdr = fake.client();
    fake.on("agent.start", Reply::Result(reply(false, "w1:p1")));
    assert!(start(&herdr, "synthetic", "w1:p1", 1).await.is_err());
    fake.on("agent.start", Reply::Result(reply(true, "w1:p2")));
    assert!(start(&herdr, "synthetic", "w1:p1", 2000).await.is_err());
}
