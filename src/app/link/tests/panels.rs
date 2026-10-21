//! The panels task: one pass at a time, the open key, quit (the core's shutdown, then every
//! panel provably ours closed), and per-tab switched off while running.

use crate::app::link::Done;
use crate::app::link::panels::{PanelsLink, PanelsTask};
use crate::core::{self, Config, Core};
use crate::home::real::RealHome;
use crate::home::{HOME_LABEL, Places};
use crate::panels::real::RealOps;
use crate::panels::{Manager, Places as PanelPlaces};
use crate::settings::Settings;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

const ID: &str = "leavemyyard.herdr-panorama";

async fn core(dir: &Path) -> Arc<Core> {
    let uid = std::fs::metadata(dir).unwrap().uid();
    let cfg = Config::new(
        dir.join("s").join("core.sock"),
        dir.join("s").join("core.lock"),
        uid,
    );
    let (tx, _rx) = core::events();
    std::mem::forget(_rx); // the app's end of the core's events, kept open
    Arc::new(core::start(cfg, Panes, tx).await.expect("the core starts"))
}

struct Panes;
impl core::Verify for Panes {
    async fn verify(&self, _pane: &str, _pid: i32) -> bool {
        false
    }
}

fn task(
    fake: &FakeHerdr,
    core: Arc<Core>,
    dir: &Path,
    config: &str,
) -> (PanelsTask, mpsc::UnboundedReceiver<Done>) {
    std::fs::create_dir_all(dir.join("cfg")).unwrap();
    std::fs::write(dir.join("cfg").join("config.toml"), config).unwrap();
    let ops = RealOps {
        herdr: fake.client(),
        plugin_id: ID.into(),
        root: None,
        width: 46,
    };
    let places = PanelPlaces {
        state_path: Some(dir.join("panels.json")),
        marker_dir: Some(dir.into()),
        home_lock: dir.join("home.lock"),
    };
    let manager = Manager::new(ops, None, places, Arc::new(Instant::now));
    let (done, rx) = mpsc::unbounded_channel();
    let t = PanelsTask {
        manager,
        core,
        swarm: Arc::new(Mutex::new(Default::default())),
        settings: Settings::new(Some(dir.join("cfg")), dir.join("herdr"), ID),
        home: RealHome {
            herdr: fake.client(),
            plugin_id: ID.into(),
            root: None,
        },
        places: Places {
            record: dir.join("home.json"),
            lock: dir.join("home.lock"),
            wait: Duration::from_secs(2),
        },
        my_pane: Some("w1:p1".into()),
        done,
        log: dir.join("panorama.log"),
        explode: Default::default(),
    };
    (t, rx)
}

fn snapshot(fake: &FakeHerdr, home_tab_label: &str) {
    fake.on("session.snapshot", Reply::Result(json!({"snapshot": {
        "focused_tab_id": "w1:t1", "focused_pane_id": "w1:p1",
        "tabs": [{"tab_id": "w1:t1", "label": home_tab_label, "workspace_id": "w1"},
                 {"tab_id": "w1:t2", "label": "billing", "workspace_id": "w1"}],
        "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "label": HOME_LABEL},
                  {"pane_id": "w1:p7", "tab_id": "w1:t2", "workspace_id": "w1", "label": "Panorama panel"},
                  {"pane_id": "w1:p2", "tab_id": "w1:t2", "workspace_id": "w1", "agent": "claude"}]}})));
}

fn runs(fake: &FakeHerdr, command: &str) {
    fake.on("pane.process_info", Reply::Result(json!({"type": "pane_process_info", "process_info": {"pane_id": "w1:p1",
        "shell_pid": 5, "foreground_processes": [{"pid": 5, "name": "panorama", "argv": ["/p/panorama", command]}]}})));
}

/// A panel viewer connected to the core: its socket, after hello.
async fn viewer(dir: &Path) -> BufReader<UnixStream> {
    let mut s = UnixStream::connect(dir.join("s").join("core.sock"))
        .await
        .unwrap();
    s.write_all(b"{\"hello\": {\"protocol\": \"1.3\"}}\n")
        .await
        .unwrap();
    let mut r = BufReader::new(s);
    let mut l = String::new();
    r.read_line(&mut l).await.unwrap();
    assert!(l.contains("welcome"));
    r
}

async fn said_shutdown(r: &mut BufReader<UnixStream>) -> bool {
    let mut l = String::new();
    while tokio::time::timeout(Duration::from_secs(3), r.read_line(&mut l))
        .await
        .is_ok_and(|n| n.is_ok_and(|n| n > 0))
    {
        if serde_json::from_str::<Value>(&l).is_ok_and(|v| v.get("shutdown").is_some()) {
            return true;
        }
        l.clear();
    }
    false
}

#[tokio::test(flavor = "current_thread")]
async fn quit_tells_the_viewers_to_go_then_closes_our_panels() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    let mut v = viewer(dir.path()).await;
    snapshot(&fake, HOME_LABEL);
    runs(&fake, "panel");
    fake.on("plugin.pane.close", Reply::Result(json!({"type": "ok"})));
    let (mut t, _rx) = task(&fake, c, dir.path(), "cockpit = true\n");
    t.manager.created.insert("w1:p7".into(), "w1:t2".into()); // a panel this core opened
    let link = PanelsLink::spawn(t);
    link.quit().await;
    assert!(said_shutdown(&mut v).await, "the viewers were told to go");
    assert_eq!(
        fake.params_of("plugin.pane.close"),
        [json!({"pane_id": "w1:p7"})]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn the_open_key_shows_the_home_view_where_it_is() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    snapshot(&fake, HOME_LABEL);
    runs(&fake, "tui");
    for m in ["tab.focus", "pane.focus"] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    let (t, _rx) = task(&fake, c, dir.path(), ""); // per-tab off: the home view
    let link = PanelsLink::spawn(t);
    assert_eq!(link.open_key().await, "home");
    assert_eq!(fake.params_of("tab.focus"), [json!({"tab_id": "w1:t1"})]);
    assert_eq!(fake.params_of("pane.focus"), [json!({"pane_id": "w1:p1"})]);
    assert!(
        fake.params_of("pane.move").is_empty(),
        "in its home tab: nothing moves"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn the_open_key_brings_a_docked_home_back_first() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    snapshot(&fake, "billing"); // the home pane sits in a tab that is no home tab
    runs(&fake, "tui");
    fake.on(
        "pane.move",
        Reply::Result(json!({"type": "pane_move", "move_result": {"changed": true,
        "pane": {"pane_id": "w1:p1", "tab_id": "w1:t9"}}})),
    );
    for m in [
        "tab.focus",
        "pane.focus",
        "tab.move",
        "tab.rename",
        "pane.rename",
    ] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    let (t, mut rx) = task(&fake, c, dir.path(), "");
    let link = PanelsLink::spawn(t);
    assert_eq!(link.open_key().await, "home");
    assert_eq!(fake.params_of("pane.move")[0]["pane_id"], "w1:p1");
    assert_eq!(
        fake.params_of("tab.focus").last().unwrap(),
        &json!({"tab_id": "w1:t9"})
    );
    assert!(matches!(rx.try_recv(), Ok(Done::MyPane(p)) if p == "w1:p1"));
}

#[tokio::test(flavor = "current_thread")]
async fn per_tab_switched_off_closes_the_panels_once() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    let mut v = viewer(dir.path()).await;
    snapshot(&fake, HOME_LABEL);
    runs(&fake, "panel");
    fake.on("plugin.pane.close", Reply::Result(json!({"type": "ok"})));
    let (mut t, mut rx) = task(&fake, c, dir.path(), "cockpit = false\n");
    t.manager.started = true;
    t.manager.created.insert("w1:p7".into(), "w1:t2".into());
    let link = PanelsLink::spawn(t);
    link.pass();
    let d = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(d, Done::Panels(m) if m.is_empty()));
    assert!(said_shutdown(&mut v).await);
    assert_eq!(fake.params_of("plugin.pane.close").len(), 1);
    link.pass(); // already off: nothing more
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(fake.params_of("plugin.pane.close").len(), 1);
    assert!(rx.try_recv().is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn no_pass_runs_before_the_first_model() {
    // a focus burst before the first collect (or after a failed one) must not spend the start
    // batch, nor run the restore check, on an empty swarm
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    snapshot(&fake, HOME_LABEL);
    runs(&fake, "panel");
    let (t, mut rx) = task(&fake, c, dir.path(), "cockpit = true\n");
    let swarm = t.swarm.clone();
    let link = PanelsLink::spawn(t);
    link.pass();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        fake.params_of("session.snapshot").is_empty(),
        "nothing looked at"
    );
    assert!(rx.try_recv().is_err());
    *swarm.lock().unwrap() = Some(Default::default()); // the first model
    link.pass();
    let d = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(d, Done::Panels(_)));
    assert!(!fake.params_of("session.snapshot").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn quit_says_shutdown_before_it_closes_anything() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    let mut v = viewer(dir.path()).await;
    snapshot(&fake, HOME_LABEL);
    runs(&fake, "panel");
    fake.on("plugin.pane.close", Reply::Hang); // the close never ends: nothing may wait for it
    let (mut t, _rx) = task(&fake, c, dir.path(), "cockpit = true\n");
    t.manager.created.insert("w1:p7".into(), "w1:t2".into());
    let link = PanelsLink::spawn(t);
    let quitting = tokio::spawn(async move { link.quit().await });
    while fake.params_of("plugin.pane.close").is_empty() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // the close is under way: the viewers have already been told to go
    let mut l = String::new();
    let got = tokio::time::timeout(Duration::from_millis(500), v.read_line(&mut l)).await;
    assert!(got.is_ok() && l.contains("shutdown"), "{l:?}");
    quitting.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn quit_closes_our_panels_before_it_releases_the_home_tab() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    fake.on("session.snapshot", Reply::Result(json!({"snapshot": {
        "tabs": [{"tab_id": "w1:t1", "label": HOME_LABEL, "workspace_id": "w1"}],
        "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "label": HOME_LABEL},
                  {"pane_id": "w1:p50", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude", "name": "billing"},
                  {"pane_id": "w1:p7", "tab_id": "w1:t2", "workspace_id": "w1", "label": "Panorama panel"}]}})));
    runs(&fake, "panel");
    for m in ["plugin.pane.close", "tab.rename"] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    let (mut t, _rx) = task(&fake, c, dir.path(), "cockpit = true\n");
    t.manager.created.insert("w1:p7".into(), "w1:t2".into());
    let mut l = super::link(&fake, true, dir.path());
    l.panels = Some(PanelsLink::spawn(t));
    l.quit().await;
    let order: Vec<String> = fake
        .requests()
        .into_iter()
        .map(|r| r.method)
        .filter(|m| m == "plugin.pane.close" || m == "tab.rename")
        .collect();
    assert_eq!(order, ["plugin.pane.close", "tab.rename"]);
}

#[test]
fn a_step_that_panics_is_logged_and_the_task_goes_on() {
    let _hook = crate::ui::guard::hook_lock(); // a pass panics on purpose
    crate::ui::guard::block_on(async {
        let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
        let c = core(dir.path()).await;
        snapshot(&fake, HOME_LABEL);
        runs(&fake, "panel");
        fake.on("plugin.pane.close", Reply::Result(json!({"type": "ok"})));
        let (mut t, mut rx) = task(&fake, c, dir.path(), "cockpit = true\n");
        t.manager.created.insert("w1:p7".into(), "w1:t2".into());
        *t.swarm.lock().unwrap() = Some(Default::default());
        t.explode.store(1, std::sync::atomic::Ordering::SeqCst);
        let link = PanelsLink::spawn(t);
        link.pass(); // panics
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            rx.try_recv().is_err(),
            "the panicking pass reported nothing"
        );
        link.pass(); // the next one runs
        let d = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("a later pass ran")
            .unwrap();
        assert!(matches!(d, Done::Panels(_)));
        tokio::time::timeout(Duration::from_secs(5), link.quit())
            .await
            .expect("quit still runs");
        assert_eq!(
            fake.params_of("plugin.pane.close"),
            [json!({"pane_id": "w1:p7"})],
            "and closes our panel"
        );
        let log = std::fs::read_to_string(dir.path().join("panorama.log")).unwrap_or_default();
        assert!(log.contains("panels: a pass failed"), "{log}");
    });
}

#[tokio::test(flavor = "current_thread")]
async fn the_open_key_follows_the_home_panes_new_id() {
    // test_cockpit.py::test_the_cores_open_key_follows_the_home_panes_new_id: the undock
    // gives panorama's pane a new id (herdr does across workspaces); the focus and the
    // report use it
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    snapshot(&fake, "billing");
    runs(&fake, "tui");
    fake.on(
        "pane.move",
        Reply::Result(json!({"type": "pane_move", "move_result": {"changed": true,
        "pane": {"pane_id": "w2:p9", "tab_id": "w2:t9"}}})),
    );
    for m in [
        "tab.focus",
        "pane.focus",
        "tab.move",
        "tab.rename",
        "pane.rename",
    ] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    let (t, mut rx) = task(&fake, c, dir.path(), "");
    let link = PanelsLink::spawn(t);
    assert_eq!(link.open_key().await, "home");
    assert_eq!(fake.params_of("pane.move")[0]["pane_id"], "w1:p1"); // the old id moved
    assert_eq!(
        fake.params_of("tab.focus").last().unwrap(),
        &json!({"tab_id": "w2:t9"})
    );
    assert_eq!(
        fake.params_of("pane.focus").last().unwrap(),
        &json!({"pane_id": "w2:p9"})
    );
    assert!(matches!(rx.try_recv(), Ok(Done::MyPane(p)) if p == "w2:p9"));
}
