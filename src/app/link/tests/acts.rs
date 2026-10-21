//! The input's herdr side for the app: going to a pane, the inbox, + New agent, zoom.

use super::{done, link, quiet};
use crate::app::link::acts::NEW_AGENT_MIN_WIDTH;
use crate::app::link::{Done, Link};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn goto_focuses_the_agent_and_a_popup_closes_then() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on("agent.focus", Reply::Result(json!({"type": "ok"})));
    let mut home = link(&fake, true, dir.path());
    home.goto("w1:p3".into());
    assert!(quiet(&mut home).await, "the home view stays");
    assert_eq!(fake.params_of("agent.focus")[0]["target"], "w1:p3");
    let mut popup = link(&fake, false, dir.path());
    popup.goto("w1:p3".into());
    assert!(matches!(done(&mut popup).await, Done::Exit));
    fake.on("agent.focus", Reply::error("agent_not_found", "no agent"));
    popup.goto("w1:p9".into());
    let Done::Toast(t) = done(&mut popup).await else {
        panic!("no notice")
    };
    assert!(
        t.text.starts_with("could not focus w1:p9: ") && t.warning,
        "{t:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn the_inbox_opens_over_the_home_view_and_after_a_popup() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on("plugin.list", Reply::Result(json!({"plugins": []})));
    let mut popup = link(&fake, false, dir.path());
    popup.open_inbox(Some(7), None);
    let Done::ExitThenInbox { question, pane } = done(&mut popup).await else {
        panic!("not after exit")
    };
    assert_eq!((question, pane), (Some(7), None));
    assert!(
        fake.params_of("plugin.list").is_empty(),
        "nothing opens while the popup is up"
    );
    let mut home = link(&fake, true, dir.path());
    home.open_inbox(None, Some("w1:p2".into()));
    assert!(quiet(&mut home).await, "the home tab is not modal: no exit");
}

#[tokio::test(flavor = "current_thread")]
async fn new_agent_uses_the_form_where_it_fits_else_the_popup() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on("plugin.pane.open", Reply::Result(json!({"type": "ok"})));
    let mut home = link(&fake, true, dir.path());
    home.new_agent(Some("/work".into()), NEW_AGENT_MIN_WIDTH);
    assert!(
        matches!(done(&mut home).await, Done::NewAgentForm(cwd) if cwd == "/work"),
        "wide enough: the form"
    );
    home.new_agent(None, 0);
    assert!(
        matches!(done(&mut home).await, Done::NewAgentForm(cwd) if cwd == "/home/op"),
        "no size yet: the form, home dir"
    );
    let mut popup = link(&fake, false, dir.path());
    popup.new_agent(None, 46);
    assert!(
        matches!(done(&mut popup).await, Done::NewAgentForm(_)),
        "a popup has its form"
    );
    home.new_agent(Some("/work".into()), 46);
    assert!(quiet(&mut home).await, "narrow home pane: the popup opened");
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(p["entrypoint"], "new-agent");
    assert_eq!(p["env"]["PANORAMA_NEW_AGENT_CWD"], "/work");
    fake.on(
        "plugin.pane.open",
        Reply::error("ui_busy", "a popup pane is already open"),
    );
    home.new_agent(Some("/work".into()), 46);
    assert!(
        matches!(done(&mut home).await, Done::NewAgentForm(_)),
        "refused: the form here"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn zoom_is_the_home_panes_own() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on(
        "pane.zoom",
        Reply::Result(json!({"type": "ok", "zoom": {}})),
    );
    let mut home = link(&fake, true, dir.path());
    home.published.focused_pane = Some("w1:p3".into()); // herdr's focus is elsewhere
    home.zoom();
    assert!(quiet(&mut home).await);
    assert_eq!(
        fake.params_of("pane.zoom")[0]["pane_id"],
        "w1:p1",
        "its own pane, never herdr's focused one"
    );
    let mut popup = link(&fake, false, dir.path());
    popup.zoom();
    assert!(quiet(&mut popup).await);
    assert_eq!(
        fake.params_of("pane.zoom").len(),
        1,
        "a popup does not zoom"
    );
    fake.on("pane.zoom", Reply::error("pane_not_found", "gone"));
    home.zoom();
    let Done::Toast(t) = done(&mut home).await else {
        panic!("no notice")
    };
    assert_eq!(t.title.as_deref(), Some("zoom"));
}

#[tokio::test(flavor = "current_thread")]
async fn quit_releases_a_shared_home_tab_after_the_panels() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on("session.snapshot", Reply::Result(json!({"snapshot": {
        "tabs": [{"tab_id": "w1:t1", "label": crate::home::HOME_LABEL, "workspace_id": "w1"}],
        "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "label": crate::home::HOME_LABEL},
                  {"pane_id": "w1:p50", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude"}],
        "agents": [{"pane_id": "w1:p50", "name": "billing"}]}})));
    fake.on("tab.rename", Reply::Result(json!({"type": "ok"})));
    let mut home = link(&fake, true, dir.path());
    home.quit().await;
    assert_eq!(
        fake.params_of("tab.rename"),
        [json!({"tab_id": "w1:t1", "label": "billing"})]
    );
    // a popup releases nothing
    let mut popup = link(&fake, false, dir.path());
    popup.quit().await;
    assert_eq!(fake.params_of("tab.rename").len(), 1);
    // panorama alone in its tab: the tab closes with it, nothing is renamed
    fake.on("session.snapshot", Reply::Result(json!({"snapshot": {
        "tabs": [{"tab_id": "w1:t1", "label": crate::home::HOME_LABEL, "workspace_id": "w1"}],
        "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "label": crate::home::HOME_LABEL}]}})));
    let mut alone = link(&fake, true, dir.path());
    alone.quit().await;
    assert_eq!(fake.params_of("tab.rename").len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn an_unanswered_popup_request_never_also_opens_the_form() {
    // tests/test_tui.py: herdr may have opened it all the same; one that never started is said
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on("plugin.pane.open", Reply::Close);
    let mut home = link(&fake, true, dir.path());
    let mark = dir.path().join("new-agent.mark");
    std::fs::write(&mark, "99999999999").unwrap(); // the popup's process marked its start
    home.new_agent(Some("/work".into()), 46);
    assert!(quiet(&mut home).await, "it started: no form, no notice");
    std::fs::remove_file(&mark).unwrap();
    home.new_agent(Some("/work".into()), 46);
    match done(&mut home).await {
        Done::Toast(t) => assert_eq!(t.text, crate::panel::act::new_agent::DID_NOT_OPEN),
        other => panic!("{other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_forms_start_reports_its_end() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut home = link(&fake, true, dir.path());
    let missing = dir.path().join("gone").to_string_lossy().into_owned();
    home.start_agent("docs".into(), Some(missing), "claude");
    match done(&mut home).await {
        Done::Started(Err(why)) => assert!(why.contains("gone"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(
        fake.requests().is_empty(),
        "refused before herdr is asked anything"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn the_forms_start_reads_per_tab_mode_from_the_config() {
    // per-tab (cockpit = true): the tab is built with its panel (layout.apply); else a plain
    // tab (tab.create). The config is read at each start.
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    );
    fake.on("layout.apply", Reply::error("boom", "no layout"));
    fake.on("tab.create", Reply::error("boom", "no tab"));
    let cfg = dir.path().join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    let mut home = link(&fake, true, dir.path());
    home.settings = Some(std::sync::Arc::new(std::sync::Mutex::new(
        crate::settings::Settings::new(Some(cfg.clone()), dir.path().join("herdr"), "p"),
    )));
    let start = |l: &mut Link| l.start_agent("docs".into(), None, "claude");
    start(&mut home);
    assert!(matches!(done(&mut home).await, Done::Started(Err(_))));
    assert_eq!(
        (
            fake.params_of("layout.apply").len(),
            fake.params_of("tab.create").len()
        ),
        (0, 1)
    );
    std::fs::write(cfg.join("config.toml"), "cockpit = true\n").unwrap();
    start(&mut home);
    assert!(matches!(done(&mut home).await, Done::Started(Err(_))));
    assert_eq!(
        (
            fake.params_of("layout.apply").len(),
            fake.params_of("tab.create").len()
        ),
        (1, 1)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_popups_start_reads_the_plugins_config_and_environment() {
    // #124 final pass L2: the settings Link::start wires in (a graph-view popup, cockpit = true)
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on(
        "agent.get",
        Reply::error("agent_not_found", "agent not found"),
    );
    fake.on("layout.apply", Reply::error("boom", "no layout"));
    fake.on("tab.create", Reply::error("boom", "no tab"));
    let cfg = dir.path().join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(cfg.join("config.toml"), "cockpit = true\n").unwrap();
    let (cfg_s, state_s) = (
        cfg.to_string_lossy().into_owned(),
        dir.path().join("state").to_string_lossy().into_owned(),
    );
    let env = herdr_agent_kit::plugin_env::PluginEnv::from_vars(
        [
            ("HERDR_PLUGIN_ID", "leavemyyard.herdr-panorama"),
            ("HERDR_PLUGIN_ROOT", "/plugins/p"),
            ("HERDR_PLUGIN_CONFIG_DIR", cfg_s.as_str()),
            ("HERDR_PLUGIN_STATE_DIR", state_s.as_str()),
        ],
        Some(dir.path().to_path_buf()),
        1000,
    );
    let mut popup =
        Link::start_with(&env, fake.client(), false, None, dir.path().join("log")).await;
    popup.start_agent("docs".into(), None, "claude");
    assert!(matches!(done(&mut popup).await, Done::Started(Err(_))));
    assert_eq!(
        fake.params_of("tab.create").len(),
        0,
        "per-tab: no plain tab"
    );
    let layout = &fake.params_of("layout.apply")[0];
    assert_eq!(
        layout["root"]["first"]["env"]["HERDR_PLUGIN_ROOT"], "/plugins/p",
        "{layout}"
    );
}

#[test]
fn a_start_that_panics_ends_as_a_failed_start() {
    // #124 final pass L1: the modal never stays busy
    let _hook = crate::ui::guard::hook_lock(); // the start panics on purpose
    crate::ui::guard::block_on(async {
        let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
        let mut home = link(&fake, true, dir.path());
        home.report_start(async { panic!("boom") });
        match done(&mut home).await {
            Done::Started(Err(why)) => assert!(
                why.starts_with("+ New agent failed:") && why.contains("boom"),
                "{why}"
            ),
            other => panic!("{other:?}"),
        }
    });
}
