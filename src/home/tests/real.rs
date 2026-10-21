//! The home module's herdr side (`RealHome`) against the kit's scripted herdr.

use crate::home::real::RealHome;
use crate::home::{At, HomeOps};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

fn ops(fake: &FakeHerdr) -> RealHome {
    RealHome {
        herdr: fake.client(),
        plugin_id: "leavemyyard.herdr-panorama".into(),
        root: Some("/plugins/p".into()),
    }
}

fn opened() -> Reply {
    Reply::Result(
        json!({"type": "plugin_pane_opened", "plugin_pane": {"plugin_id": "leavemyyard.herdr-panorama",
        "entrypoint": "panorama", "pane": {"pane_id": "w1:p4", "tab_id": "w1:t4"}}}),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn the_home_opens_in_a_tab_of_its_own_marked_home() {
    let fake = FakeHerdr::start();
    fake.on("plugin.pane.open", opened());
    let at = ops(&fake).open_home_pane(false).await.unwrap();
    assert_eq!(
        at,
        At {
            pane_id: "w1:p4".into(),
            tab_id: "w1:t4".into()
        }
    );
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(p["entrypoint"], "panorama");
    assert_eq!(p["placement"], "tab");
    assert_eq!(p["focus"], false);
    assert_eq!(p["env"], json!({"PANORAMA_HOME": "1"}));
    assert_eq!(p["cwd"], "/plugins/p");
    ops(&fake).open_home_pane(true).await.unwrap();
    assert_eq!(fake.params_of("plugin.pane.open")[1]["focus"], true);
    fake.on("plugin.pane.open", Reply::Result(json!({"type": "ok"})));
    assert!(
        ops(&fake).open_home_pane(true).await.is_err(),
        "no pane in the reply"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn the_popup_opens_at_the_entrypoints_own_placement() {
    let fake = FakeHerdr::start();
    fake.on("plugin.pane.open", Reply::Result(json!({"type": "ok"})));
    ops(&fake).open_popup().await.unwrap();
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(p["entrypoint"], "panorama");
    assert!(p.get("placement").is_none() && p.get("env").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn the_home_moves_to_a_new_first_tab_without_focus() {
    let fake = FakeHerdr::start();
    fake.on(
        "pane.move",
        Reply::Result(json!({"type": "pane_move", "move_result": {"changed": true,
        "pane": {"pane_id": "w1:p1", "tab_id": "w1:t9"}, "previous_pane_id": "w1:p1"}})),
    );
    for m in ["tab.move", "pane.rename", "tab.rename", "tab.focus"] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    let o = ops(&fake);
    let at = o
        .move_to_new_tab("w1:p1", "w1", "◆ panorama")
        .await
        .unwrap();
    assert_eq!(at.tab_id, "w1:t9");
    let mv = &fake.params_of("pane.move")[0];
    assert_eq!(mv["pane_id"], "w1:p1");
    assert_eq!(mv["focus"], false);
    let dest = mv.get("destination").unwrap_or(mv);
    assert_eq!(dest["type"], "new_tab");
    assert_eq!(dest["workspace_id"], "w1");
    assert_eq!(dest["label"], "◆ panorama");
    o.move_tab_first("w1:t9").await.unwrap();
    assert_eq!(
        fake.params_of("tab.move")[0],
        json!({"tab_id": "w1:t9", "insert_index": 0})
    );
    o.rename_pane("w1:p1", "◆ panorama").await.unwrap();
    o.rename_tab("w1:t9", "◆ panorama").await.unwrap();
    assert_eq!(fake.params_of("pane.rename")[0]["label"], "◆ panorama");
    assert_eq!(fake.params_of("tab.rename")[0]["tab_id"], "w1:t9");
    o.focus_tab("w1:t9").await.unwrap();
    assert_eq!(fake.params_of("tab.focus")[0]["tab_id"], "w1:t9");
}

#[tokio::test(flavor = "current_thread")]
async fn the_home_pane_is_known_by_its_process() {
    let fake = FakeHerdr::start();
    let info = |argv: &[&str]| {
        Reply::Result(
            json!({"type": "pane_process_info", "process_info": {"pane_id": "w1:p1", "shell_pid": 5,
            "foreground_processes": [{"pid": 5, "name": "x", "argv": argv}]}}),
        )
    };
    fake.on(
        "pane.process_info",
        info(&["/p/.venv/bin/python3", "/p/.venv/bin/panorama", "tui"]),
    );
    assert_eq!(ops(&fake).runs_tui("w1:p1").await, Some(true));
    fake.on(
        "pane.process_info",
        info(&["/p/.venv/bin/panorama", "panel"]),
    );
    assert_eq!(
        ops(&fake).runs_tui("w1:p1").await,
        Some(false),
        "a per-tab panel is no home"
    );
    fake.on(
        "pane.process_info",
        Reply::error("pane_not_found", "pane not found"),
    );
    assert_eq!(ops(&fake).runs_tui("w1:p1").await, None);
}

#[tokio::test(flavor = "current_thread")]
async fn herdrs_snapshot_gives_the_tabs_and_agent_names() {
    let fake = FakeHerdr::start();
    fake.replay("api.jsonl", 0);
    let snap = ops(&fake).snapshot().await.unwrap();
    let tabs: Vec<(String, String)> = snap
        .tabs
        .iter()
        .map(|t| (t.tab_id.clone(), t.label.clone().unwrap_or_default()))
        .collect();
    assert_eq!(
        tabs,
        [
            ("w1:t1".into(), "1".into()),
            ("w1:t2".into(), "Agent".into()),
            ("w1:t3".into(), "Other".into())
        ]
    );
    assert_eq!(snap.tab_label("w1:t2"), "Agent");
    assert_eq!(
        snap.in_tab("w1:t2")
            .map(|p| p.pane_id.as_str())
            .collect::<Vec<_>>(),
        ["w1:p2"]
    );
    assert!(
        snap.tabs
            .iter()
            .all(|t| t.workspace_id.as_deref() == Some("w1"))
    );
}
