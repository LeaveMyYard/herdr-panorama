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

#[test]
fn a_spawned_thread_carries_its_spawner_given_task_name() {
    let shapes: serde_json::Value =
        serde_json::from_str(include_str!("spawn_shapes.json")).unwrap();
    let child = parse::thread(&shapes["child_thread"], "child-thread").unwrap();
    assert_eq!(child.task_name.as_deref(), Some("review auth flow"));
    assert_eq!(child.nickname.as_deref(), Some("Euclid"));
    assert_eq!(child.role.as_deref(), Some("explorer"));
    assert_eq!(child.name, None);
    let mut ids = Vec::new();
    parse::child_ids(&shapes["spawn_item"]["item"], &mut ids);
    assert_eq!(ids, ["child-thread"]);
    // An unnamed (v1) spawn, the root's own path and a malformed source give no task name.
    for source in [
        json!({"subAgent":{"thread_spawn":{"parent_thread_id":"p","depth":1,"agent_path":null}}}),
        json!({"subAgent":{"thread_spawn":{"parent_thread_id":"p","depth":1,"agent_path":"/root"}}}),
        json!({"subAgent":{"thread_spawn":{"parent_thread_id":"p","depth":1,"agent_path":"/root/a/"}}}),
        json!("cli"),
    ] {
        let mut v = shapes["child_thread"].clone();
        v["source"] = source.clone();
        let t = parse::thread(&v, "child-thread").unwrap();
        let expect = (source.to_string().contains("/root/a/")).then_some("a");
        assert_eq!(t.task_name.as_deref(), expect, "{source}");
    }
}
