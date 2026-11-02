use crate::codex::{Options, TurnStatus, parse};
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::json;
#[test]
fn unknown_idle_notloaded_never_mean_completed() {
    for status in ["idle", "notLoaded", "futureState", ""] {
        assert_eq!(
            parse::turn(&json!({"id":"t","status":status}))
                .unwrap()
                .status,
            TurnStatus::Unknown
        );
    }
    for status in ["idle", "notLoaded", "active"] {
        let thread = parse::thread(
            &json!({"id":"r","status":{"type":status},"sessionId":"tree","parentThreadId":null}),
            "r",
        )
        .unwrap();
        assert!(thread.turns.is_empty());
    }
}
#[test]
fn exact_ids_are_validated_and_display_text_is_bounded() {
    for id in ["", "../session", "with space", "a\n"] {
        assert!(!parse::valid_id(id));
    }
    assert!(parse::thread(&json!({"id":"tree"}), "thread").is_none());
    assert_eq!(parse::clean(&"a".repeat(400)).len(), 240);
    assert_eq!(parse::clean("a\u{1b}\nb"), "ab");
}
#[test]
fn codex_home_is_explicit_or_under_home_never_cwd() {
    let env = PluginEnv::from_vars(
        [("CODEX_HOME", "/private/codex"), ("HOME", "/private/home")],
        None,
        1000,
    );
    assert_eq!(
        Options::from_env(&env).unwrap().codex_home,
        std::path::Path::new("/private/codex")
    );
}

#[test]
fn parentlessness_requires_explicit_null_not_invalid_or_missing_evidence() {
    for parent in [
        json!({"unexpected":"parent"}),
        json!(12),
        json!("../bad"),
        json!(""),
    ] {
        assert!(parse::thread(&json!({"id":"root","parentThreadId":parent}), "root").is_none());
    }
    assert!(parse::thread(&json!({"id":"root"}), "root").is_none());
    assert!(
        parse::thread(&json!({"id":"root","parentThreadId":null}), "root")
            .unwrap()
            .parent_thread_id
            .is_none()
    );
}
