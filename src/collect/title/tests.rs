use super::*;

/// A subagent transcript's first record as Claude Code writes it (the demo scenario's
/// `agent-a1author…jsonl`, captured shape), with `content` replaced.
fn opening(content: serde_json::Value) -> String {
    let mut rec: serde_json::Value = serde_json::from_str(
        r#"{"parentUuid":null,"isSidechain":true,"promptId":"00000000-0000-4000-8000-000000000000","agentId":"a1author0000000001","type":"user","message":{"role":"user","content":"brief"},"uuid":"e34645e8-e0eb-41ba-8cee-59d1faae3b5a","timestamp":"2026-10-14T09:02:00.000Z","userType":"external","entrypoint":"cli","cwd":"/home/user/project","sessionId":"11111111-1111-4111-8111-111111111111","version":"","gitBranch":""}"#,
    )
    .unwrap();
    rec["message"]["content"] = content;
    rec.to_string() + "\n{\"type\":\"assistant\"}\n"
}

fn transcript(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent-a1author0000000001.jsonl");
    std::fs::write(&path, text).unwrap();
    (dir, path)
}

#[test]
fn cleaning_drops_controls_bidi_and_decorations_and_cuts_long_titles() {
    assert_eq!(
        clean("  ✳ Fix\tthe \u{202E}bug\u{7}  now "),
        "Fix the bug now"
    );
    assert_eq!(clean("❓ — "), "");
    let long = clean(&"word ".repeat(40));
    assert_eq!(long.chars().count(), MAX);
    assert!(long.ends_with('…'));
}

#[test]
fn a_subagent_is_named_by_its_spawner_first() {
    let never = || -> Option<String> { panic!("the prompt is read only when needed") };
    let t = claude_agent(
        Some("Author PR #42"),
        Some("other"),
        never,
        None,
        "a1b2c3d4",
    );
    assert_eq!(t, ("Author PR #42".into(), "spawner"));
    let t = claude_agent(
        None,
        Some("Review the diff"),
        never,
        Some("Explore"),
        "a1b2c3d4",
    );
    assert_eq!(t, ("Review the diff".into(), "spawner"));
    // a description that cleans to nothing does not count
    let t = claude_agent(
        Some(" ✳ "),
        Some(""),
        || Some("Do it".into()),
        None,
        "a1b2c3d4",
    );
    assert_eq!(t, ("Do it".into(), "prompt"));
}

#[test]
fn without_a_description_its_prompt_then_type_then_id_name_it() {
    let none = || None;
    let t = claude_agent(None, None, none, Some("Explore"), "a1b2c3d4e5");
    assert_eq!(t, ("Explore a1b2c3d".into(), "type"));
    let t = claude_agent(None, None, none, None, "a1b2c3d4e5");
    assert_eq!(t, ("agent a1b2c3d".into(), "id"));
}

#[test]
fn the_first_prompt_is_the_first_line_of_the_opening_user_record() {
    let (_d, p) = transcript(&opening(serde_json::json!(
        "\n  Find the flaky test\nthen fix it"
    )));
    assert_eq!(first_prompt(&p).as_deref(), Some("Find the flaky test"));
    let blocks = serde_json::json!([{"type": "image"}, {"type": "text", "text": "Read the logs"}]);
    let (_d, p) = transcript(&opening(blocks));
    assert_eq!(first_prompt(&p).as_deref(), Some("Read the logs"));
    let (_d, p) = transcript("{\"type\":\"assistant\"}\n");
    assert_eq!(first_prompt(&p), None);
    assert_eq!(first_prompt(Path::new("/nonexistent/agent.jsonl")), None);
}

fn inputs<'a>() -> RootInputs<'a> {
    RootInputs {
        kind: "claude",
        pane_name: None,
        tab_label: "",
        per_tab: 1,
        session: [None; 3],
        first_prompt: None,
        terminal_title: None,
    }
}

#[test]
fn an_operator_named_pane_or_tab_wins_over_the_session() {
    let session = [Some("custom"), Some("agent"), Some("ai")];
    let r = RootInputs {
        pane_name: Some("reviewer pane"),
        tab_label: "Billing API",
        session,
        ..inputs()
    };
    assert_eq!(root(&r), Some(("reviewer pane".into(), "pane")));
    let r = RootInputs {
        tab_label: "Billing API",
        session,
        ..inputs()
    };
    assert_eq!(root(&r), Some(("Billing API".into(), "tab")));
}

#[test]
fn a_neutral_tab_label_does_not_name_a_root() {
    for label in ["", "3", "claude", "Codex", "tab", "Claude Code", "mybot"] {
        let r = RootInputs {
            pane_name: (label == "mybot").then_some("mybot"),
            tab_label: label,
            session: [None, None, Some("Ai title")],
            ..inputs()
        };
        let want = if label == "mybot" {
            ("mybot".into(), "pane") // the pane name wins; the tab only echoes it
        } else {
            ("Ai title".into(), "session")
        };
        assert_eq!(root(&r), Some(want), "label {label:?}");
    }
}

#[test]
fn a_tab_with_several_agents_names_none_of_them() {
    let r = RootInputs {
        tab_label: "Billing API",
        per_tab: 2,
        first_prompt: Some("Ship the export"),
        ..inputs()
    };
    assert_eq!(root(&r), Some(("Ship the export".into(), "prompt")));
}

#[test]
fn session_titles_go_custom_then_agent_name_then_ai_title() {
    let r = |session| {
        root(&RootInputs {
            session,
            ..inputs()
        })
    };
    assert_eq!(
        r([Some("Renamed"), Some("agent"), Some("ai")]),
        Some(("Renamed".into(), "session"))
    );
    assert_eq!(
        r([None, Some("agent"), Some("ai")]),
        Some(("agent".into(), "session"))
    );
    assert_eq!(r([None, None, Some("ai")]), Some(("ai".into(), "session")));
}

#[test]
fn then_the_first_prompt_then_a_telling_terminal_title() {
    let r = RootInputs {
        first_prompt: Some("\n Fix the build\n\nContext: the CI log says X"),
        terminal_title: Some("✳ Something else"),
        ..inputs()
    };
    assert_eq!(root(&r), Some(("Fix the build".into(), "prompt")));
    let term = |t| {
        root(&RootInputs {
            terminal_title: Some(t),
            ..inputs()
        })
    };
    assert_eq!(
        term("✳ Refactor auth"),
        Some(("Refactor auth".into(), "pane"))
    );
    for dull in [
        "~/Code/app",
        "/home/user",
        "C:\\Users",
        "user@host: ~",
        "Claude Code",
    ] {
        assert_eq!(term(dull), None, "{dull:?}");
    }
}

#[test]
fn the_last_rung_names_the_tab_else_the_pane() {
    assert_eq!(
        root_fallback("claude", Some(3), "w1:p4"),
        ("Claude · tab 3".into(), "id")
    );
    assert_eq!(
        root_fallback("claude", None, "w1:p4"),
        ("Claude · w1:p4".into(), "id")
    );
}

#[test]
fn a_codex_subagent_goes_task_name_then_name_then_nickname_then_prompt() {
    let id = "019a2b3c-0000-7000-8000-000000000001";
    let all = |task, name, nick, prompt, role| codex_agent(task, name, nick, prompt, role, id);
    let prompt = Some("\nReview the auth flow\nRead src/auth.");
    assert_eq!(
        all(
            Some("review auth"),
            Some("Named"),
            Some("Euclid"),
            prompt,
            Some("explorer")
        ),
        ("review auth".into(), "spawner")
    );
    assert_eq!(
        all(
            None,
            Some("Named"),
            Some("Euclid"),
            prompt,
            Some("explorer")
        ),
        ("Named".into(), "session")
    );
    assert_eq!(
        all(Some(" "), None, Some("Euclid"), prompt, Some("explorer")),
        ("Euclid".into(), "session")
    );
    assert_eq!(
        all(None, None, None, prompt, Some("explorer")),
        ("Review the auth flow".into(), "prompt")
    );
    assert_eq!(
        all(None, None, None, None, Some("explorer")),
        ("explorer 019a2b3".into(), "type")
    );
    assert_eq!(
        all(None, None, None, None, None),
        ("agent 019a2b3".into(), "id")
    );
}

#[test]
fn a_codex_root_takes_herdr_names_then_its_thread_name_then_its_prompt() {
    let codex = || RootInputs {
        kind: "codex",
        session: [Some("Codex rename"), None, None],
        first_prompt: Some("Fix the flaky test\nmore"),
        ..inputs()
    };
    let r = RootInputs {
        tab_label: "Billing API",
        ..codex()
    };
    assert_eq!(root(&r), Some(("Billing API".into(), "tab")));
    let r = RootInputs {
        tab_label: "codex",
        ..codex()
    };
    assert_eq!(root(&r), Some(("Codex rename".into(), "session")));
    let r = RootInputs {
        session: [None; 3],
        ..codex()
    };
    assert_eq!(root(&r), Some(("Fix the flaky test".into(), "prompt")));
}
