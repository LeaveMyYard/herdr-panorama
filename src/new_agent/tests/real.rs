//! Starting an agent against the kit's scripted herdr, the replies shaped as herdr 0.9.3
//! answered on an isolated server (the kit's recordings: agent.jsonl, socket.jsonl, tab.jsonl,
//! pane.jsonl).

use crate::new_agent::KIND;
use crate::new_agent::ops::{NewTab, StartOps};
use crate::new_agent::real::{PANEL_COMMAND, PANEL_RATIO, RealStart, START_TIMEOUT_MS};
use crate::new_agent::start::new_agent;
use crate::panels::real::RealOps;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

fn ops(fake: &FakeHerdr) -> RealStart {
    RealStart {
        herdr: fake.client(),
        plugin_env: vec![
            ("HERDR_PLUGIN_ROOT".into(), "/plugins/p".into()),
            (
                "HERDR_PLUGIN_ID".into(),
                "leavemyyard.herdr-panorama".into(),
            ),
        ],
        root: Some("/plugins/p".into()),
        panels: RealOps {
            herdr: fake.client(),
            plugin_id: "leavemyyard.herdr-panorama".into(),
            root: None,
            width: 46,
        },
        start_timeout_ms: START_TIMEOUT_MS,
    }
}

fn started() -> Reply {
    Reply::Result(
        json!({"type": "agent_started", "agent": {"name": "docs", "agent_status": "idle", "workspace_id": "w1",
        "tab_id": "w1:t5", "pane_id": "w1:p8", "focused": false, "state_change_seq": 0}, "argv": ["claude"]}),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn the_panel_layout_puts_our_panel_left_of_the_new_pane() {
    let fake = FakeHerdr::start();
    fake.on(
        "layout.apply",
        Reply::Result(
            json!({"type": "layout_apply", "layout": {"workspace_id": "w1", "tab_id": "w1:t5",
        "root": {"type": "split", "direction": "right", "ratio": 0.3,
                 "first": {"type": "pane", "pane_id": "w1:p7", "label": "Panorama panel"},
                 "second": {"type": "pane", "pane_id": "w1:p8"}}}}),
        ),
    );
    let tab = ops(&fake)
        .panel_layout("docs", Some("/work"))
        .await
        .unwrap();
    assert_eq!(
        tab,
        NewTab {
            tab: "w1:t5".into(),
            pane: Some("w1:p8".into()),
            panel: Some("w1:p7".into())
        }
    );
    let p = &fake.params_of("layout.apply")[0];
    assert_eq!(p["tab_label"], "docs");
    assert_eq!(
        p["focus"], false,
        "the operator stays where they are until the agent runs"
    );
    let root = &p["root"];
    assert_eq!(
        (
            root["type"].as_str(),
            root["direction"].as_str(),
            root["ratio"].as_f64()
        ),
        (Some("split"), Some("right"), Some(PANEL_RATIO))
    );
    assert_eq!(root["first"]["command"], json!(PANEL_COMMAND));
    assert_eq!(root["first"]["label"], "Panorama panel");
    assert_eq!(root["first"]["cwd"], "/plugins/p");
    assert_eq!(
        root["first"]["env"],
        json!({"HERDR_PLUGIN_ROOT": "/plugins/p", "HERDR_PLUGIN_ID": "leavemyyard.herdr-panorama"})
    );
    assert_eq!(root["second"], json!({"type": "pane", "cwd": "/work"}));
    // the manifest's panel command is this very one
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/herdr-plugin.toml")).unwrap();
    assert!(
        manifest.contains(r#"["sh","-c","exec sh \"${HERDR_PLUGIN_ROOT:-.}/scripts/panel\""]"#)
            || manifest
                .contains(r#"["sh", "-c", "exec sh \"${HERDR_PLUGIN_ROOT:-.}/scripts/panel\""]"#)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_plain_tab_is_made_unfocused_and_its_pane_found() {
    let fake = FakeHerdr::start();
    fake.on(
        "tab.create",
        Reply::Result(
            json!({"type": "tab_created", "tab": {"tab_id": "w1:t2", "workspace_id": "w1",
        "label": "docs", "focused": false}, "root_pane": {"pane_id": "w1:p2", "tab_id": "w1:t2"}}),
        ),
    );
    fake.on("pane.list", Reply::Result(json!({"type": "pane_list", "panes": [
        {"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1"}, {"pane_id": "w1:p2", "tab_id": "w1:t2", "workspace_id": "w1"}]})));
    let tab = ops(&fake).plain_tab("docs", None).await.unwrap();
    assert_eq!(
        tab,
        NewTab {
            tab: "w1:t2".into(),
            pane: Some("w1:p2".into()),
            panel: None
        }
    );
    let p = &fake.params_of("tab.create")[0];
    assert_eq!(
        (p["label"].as_str(), p["focus"].as_bool()),
        (Some("docs"), Some(false))
    );
    assert!(p.get("cwd").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn the_whole_start_through_herdr() {
    let fake = FakeHerdr::start();
    fake.on(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    );
    fake.on(
        "tab.create",
        Reply::Result(json!({"type": "tab_created", "tab": {"tab_id": "w1:t5"}})),
    );
    fake.on(
        "pane.list",
        Reply::Result(
            json!({"type": "pane_list", "panes": [{"pane_id": "w1:p8", "tab_id": "w1:t5"}]}),
        ),
    );
    fake.on("agent.start", started());
    fake.on("tab.focus", Reply::Result(json!({"type": "ok"})));
    assert_eq!(
        new_agent(&ops(&fake), "Docs", None, KIND, false).await,
        Ok("docs".into())
    );
    let s = &fake.params_of("agent.start")[0];
    assert_eq!(
        s,
        &json!({"name": "docs", "kind": "claude", "pane_id": "w1:p8", "timeout_ms": START_TIMEOUT_MS})
    );
    assert_eq!(fake.params_of("tab.focus"), [json!({"tab_id": "w1:t5"})]);
    assert!(fake.params_of("agent.prompt").is_empty(), "no first prompt");
    // an existing agent by that name: refused before any tab is made
    fake.on(
        "agent.get",
        Reply::Result(json!({"type": "agent_info", "agent": {"name": "docs", "pane_id": "w1:p8"}})),
    );
    assert_eq!(
        new_agent(&ops(&fake), "docs", None, KIND, false).await,
        Err("an agent named docs already exists".into())
    );
    assert_eq!(fake.params_of("tab.create").len(), 1);
    // herdr refuses the start: the tab it made is closed
    fake.on(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    );
    fake.on("agent.start", Reply::error("invalid_agent", "no such kind"));
    fake.on("tab.close", Reply::Result(json!({"type": "ok"})));
    let err = new_agent(&ops(&fake), "other", None, KIND, false)
        .await
        .unwrap_err();
    assert!(err.starts_with("agent start failed"), "{err}");
    assert_eq!(fake.params_of("tab.close"), [json!({"tab_id": "w1:t5"})]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_start_that_times_out_while_herdr_still_brings_the_agent_up_keeps_the_tab() {
    // herdr took the start but the agent was not ready in time; it is there all the same
    let fake = FakeHerdr::start();
    let pending = json!({"type": "agent_info", "agent": {"name": "slow", "pane_id": "w1:p8", "tab_id": "w1:t5",
        "agent_status": "unknown", "launch_pending": true}});
    fake.once(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    ); // before the start
    fake.on("agent.get", Reply::Result(pending.clone())); // the polls, and the look after
    fake.on(
        "tab.create",
        Reply::Result(json!({"type": "tab_created", "tab": {"tab_id": "w1:t5"}})),
    );
    fake.on(
        "pane.list",
        Reply::Result(
            json!({"type": "pane_list", "panes": [{"pane_id": "w1:p8", "tab_id": "w1:t5"}]}),
        ),
    );
    fake.on(
        "agent.start",
        Reply::Result(json!({"type": "agent_started", "agent": pending["agent"]})),
    );
    fake.on("tab.close", Reply::Result(json!({"type": "ok"})));
    fake.on("tab.focus", Reply::Result(json!({"type": "ok"})));
    let o = RealStart {
        start_timeout_ms: 300,
        ..ops(&fake)
    };
    assert_eq!(
        new_agent(&o, "slow", None, KIND, false).await,
        Ok("slow".into())
    );
    assert!(
        fake.params_of("tab.close").is_empty(),
        "a tab with the agent in it is never closed"
    );
    // and one that never came up is closed, as before
    let fake = FakeHerdr::start();
    fake.on(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    );
    fake.on(
        "tab.create",
        Reply::Result(json!({"type": "tab_created", "tab": {"tab_id": "w1:t5"}})),
    );
    fake.on(
        "pane.list",
        Reply::Result(
            json!({"type": "pane_list", "panes": [{"pane_id": "w1:p8", "tab_id": "w1:t5"}]}),
        ),
    );
    fake.on(
        "agent.start",
        Reply::Result(json!({"type": "agent_started", "agent": pending["agent"]})),
    );
    fake.on("tab.close", Reply::Result(json!({"type": "ok"})));
    let o = RealStart {
        start_timeout_ms: 300,
        ..ops(&fake)
    };
    assert!(
        new_agent(&o, "slow", None, KIND, false)
            .await
            .unwrap_err()
            .starts_with("agent start failed")
    );
    assert_eq!(fake.params_of("tab.close"), [json!({"tab_id": "w1:t5"})]);
}
