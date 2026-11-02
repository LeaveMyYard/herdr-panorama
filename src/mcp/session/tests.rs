use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn tool_prefers_the_panes_current_session() {
    let fake = FakeHerdr::start();
    // captured: `pane get` of an agent pane that reported no session (herdr 0.9.3)
    fake.replay("pane.jsonl", 4);
    let s = Sessions {
        pane: Some((fake.client(), "w1:p2".into())),
        env: Some("stale".into()),
    };
    assert_eq!(s.current().await.as_deref(), Some("stale")); // falls back to the environment
    let params = fake.params_of("pane.get");
    assert_eq!(params, [json!({"pane_id": "w1:p2"})]);
    // the same pane once its agent reported a session (the snapshot's agent_session shape)
    fake.on(
        "pane.get",
        Reply::Result(json!({"type": "pane_info", "pane": {"pane_id": "w1:p2", "agent_session":
            {"agent": "claude", "kind": "id", "source": "herdr:claude", "value": "11111111-1111-4111-8111-111111111111"}}})),
    );
    assert_eq!(
        s.current().await.as_deref(),
        Some("11111111-1111-4111-8111-111111111111")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn tool_uses_the_session_from_the_environment() {
    let none = Sessions {
        pane: None,
        env: None,
    };
    assert_eq!(none.current().await, None);
    let empty = Sessions {
        pane: None,
        env: Some(String::new()),
    };
    assert_eq!(empty.current().await, None);
    let env = Sessions {
        pane: None,
        env: Some("s2".into()),
    };
    assert_eq!(env.current().await.as_deref(), Some("s2"));
}

#[tokio::test(flavor = "current_thread")]
async fn codex_metadata_is_per_call_and_never_reads_or_registers_the_pane() {
    let fake = FakeHerdr::start();
    let sessions = Sessions {
        pane: Some((fake.client(), "w1:p2".into())),
        env: Some("claude".into()),
    };
    for id in [
        "11111111-1111-4111-8111-111111111111",
        "22222222-2222-4222-8222-222222222222",
        "11111111-1111-4111-8111-111111111111",
    ] {
        let meta = json!({"threadId":id,"sessionId":"shared-root"});
        assert_eq!(
            sessions
                .for_call(meta.as_object().unwrap())
                .await
                .unwrap()
                .as_deref(),
            Some(id)
        );
    }
    for id in [
        json!(null),
        json!(17),
        json!(""),
        json!("../root"),
        json!("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"),
    ] {
        let meta = json!({"threadId":id});
        assert!(sessions.for_call(meta.as_object().unwrap()).await.is_err());
    }
    assert!(fake.params_of("pane.get").is_empty());
    assert!(fake.params_of("pane.report_agent_session").is_empty());
    let meta = json!({"sessionId":"not-an-exact-thread"});
    assert_eq!(
        sessions
            .for_call(meta.as_object().unwrap())
            .await
            .unwrap()
            .as_deref(),
        Some("claude")
    );
}
