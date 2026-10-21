use super::super::real::invocation_pane;
use super::super::*;
use crate::core::{self, Config, Event};
use crate::home::Places;
use crate::home::real::RealHome;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::Duration;

struct NoPanels;
impl core::Verify for NoPanels {
    async fn verify(&self, _pane: &str, _pid: i32) -> bool {
        false
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_open_key_asks_the_core() {
    // tests/test_panels.py::test_the_open_key_asks_the_core
    let dir = tempfile::tempdir().unwrap();
    let uid = std::fs::metadata(dir.path()).unwrap().uid();
    let sock = dir.path().join("s").join("core.sock");
    assert_eq!(
        request_open(&sock, uid, Duration::from_secs(1)).await,
        None,
        "no core"
    );
    let cfg = Config::new(sock.clone(), dir.path().join("s").join("core.lock"), uid);
    let (tx, mut rx) = core::events();
    let c = core::start(cfg, NoPanels, tx)
        .await
        .expect("the core starts");
    let answering = tokio::spawn(async move {
        let mut asked = vec![];
        while let Some(e) = rx.recv().await {
            if let Event::Open { req, reply } = e {
                asked.push(req);
                let _ = reply.send(if asked.len() == 1 {
                    "panel".into()
                } else {
                    String::new()
                });
            }
        }
        asked
    });
    assert_eq!(
        request_open(&sock, uid, Duration::from_secs(3))
            .await
            .as_deref(),
        Some("panel")
    );
    assert_eq!(
        request_open(&sock, uid, Duration::from_secs(3))
            .await
            .as_deref(),
        Some("failed"),
        "no result"
    );
    assert_eq!(
        request_open(&sock, uid + 1, Duration::from_secs(1)).await,
        None,
        "another user's core"
    );
    c.shutdown(Duration::from_secs(2)).await;
    drop(c);
    let asked = tokio::time::timeout(Duration::from_secs(5), answering)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(asked, [serde_json::Map::new(), serde_json::Map::new()]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_core_that_never_answers_is_no_core() {
    let dir = tempfile::tempdir().unwrap();
    let uid = std::fs::metadata(dir.path()).unwrap().uid();
    let sock = dir.path().join("hang.sock");
    let _l = tokio::net::UnixListener::bind(&sock).unwrap(); // accepts, never answers
    let t = std::time::Instant::now();
    assert_eq!(
        request_open(&sock, uid, Duration::from_millis(300)).await,
        None
    );
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

fn real(fake: &FakeHerdr, from: Option<&str>) -> RealOpen {
    RealOpen {
        home: RealHome {
            herdr: fake.client(),
            plugin_id: "leavemyyard.herdr-panorama".into(),
            root: Some("/plugins/panorama".into()),
        },
        places: Places {
            record: "/nonexistent/r".into(),
            lock: "/nonexistent/l".into(),
            wait: Duration::ZERO,
        },
        core: (Path::new("/nonexistent/core.sock").to_path_buf(), 0),
        invoked_from: from.map(str::to_owned),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn open_view_split_targets_the_invoking_pane() {
    // tests/test_herdr.py::test_open_view_split_targets_the_invoking_pane
    let fake = FakeHerdr::start();
    fake.on(
        "plugin.pane.open",
        Reply::Result(json!({"type": "plugin_pane_opened"})),
    );
    real(&fake, Some("w1:p9")).open_view(true).await.unwrap();
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(
        (p["plugin_id"].as_str(), p["entrypoint"].as_str()),
        (Some("leavemyyard.herdr-panorama"), Some("panorama"))
    );
    assert_eq!(
        (p["placement"].as_str(), p["direction"].as_str()),
        (Some("split"), Some("right"))
    );
    assert_eq!(
        (p["target_pane_id"].as_str(), p["focus"].as_bool()),
        (Some("w1:p9"), Some(true))
    );
    assert_eq!(p["cwd"], "/plugins/panorama");
    // the popup: the manifest's placement (none given)
    real(&fake, Some("w1:p9")).open_view(false).await.unwrap();
    let p = &fake.params_of("plugin.pane.open")[1];
    assert!(p.get("placement").is_none_or(|v| v.is_null()), "{p}");
    assert!(p.get("target_pane_id").is_none_or(|v| v.is_null()), "{p}");
    fake.on("plugin.pane.open", Reply::error("plugin_not_found", "nope"));
    assert!(
        real(&fake, None)
            .open_view(false)
            .await
            .unwrap_err()
            .contains("nope")
    );
}

#[test]
fn the_invoking_pane_is_herdrs_focused_pane() {
    assert_eq!(
        invocation_pane(Some(r#"{"focused_pane_id": "w1:p9"}"#)).as_deref(),
        Some("w1:p9")
    );
    assert_eq!(invocation_pane(Some(r#"{"focused_pane_id": ""}"#)), None);
    assert_eq!(invocation_pane(Some("not json")), None);
    assert_eq!(invocation_pane(Some("[]")), None);
    assert_eq!(invocation_pane(None), None);
}
