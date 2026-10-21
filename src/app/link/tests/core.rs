//! tests/test_core.py's home side: every refresh the home view applies goes out to the
//! panels as a full state, and a panel's `seen` comes back to the view (the actor then
//! clears it, [`crate::app::driver::Driver::seen_in_a_panel`]).

use super::link;
use crate::app::link::LinkEvent;
use crate::core::{self, Config, Event};
use herdr_agent_kit::herdr_api::fake::FakeHerdr;
use panorama_core::view::{Filter, Unseen};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct Anyone;
impl core::Verify for Anyone {
    async fn verify(&self, _pane: &str, _pid: i32) -> bool {
        false
    }
}

async fn line(r: &mut BufReader<UnixStream>) -> Value {
    let mut l = String::new();
    tokio::time::timeout(Duration::from_secs(5), r.read_line(&mut l))
        .await
        .expect("a line in time")
        .unwrap();
    serde_json::from_str(&l).unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn the_home_view_publishes_each_refresh_and_takes_seen_back() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let uid = std::fs::metadata(dir.path()).unwrap().uid();
    let sock = dir.path().join("s").join("core.sock");
    let cfg = Config::new(sock.clone(), dir.path().join("s").join("core.lock"), uid);
    let (tx, rx) = core::events();
    let c = Arc::new(core::start(cfg, Anyone, tx).await.expect("the core starts"));
    let mut l = link(&fake, true, dir.path());
    l.core = Some(c.clone());
    l.core_events = Some(rx);
    let mut s = UnixStream::connect(&sock).await.unwrap();
    s.write_all(b"{\"hello\": {\"protocol\": \"1.3\"}}\n")
        .await
        .unwrap();
    let mut v = BufReader::new(s);
    assert!(line(&mut v).await.get("welcome").is_some());
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-design9.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let sw: panorama_core::model::Swarm = serde_json::from_value(fx["swarm"].clone()).unwrap();
    let marks = [("a5".to_owned(), Unseen::Done)].into();
    l.refreshed(&sw, &marks, Filter::All);
    let state = loop {
        let m = line(&mut v).await;
        if let Some(st) = m.get("state") {
            break st.clone();
        }
    };
    assert_eq!(state["unseen"], json!({"a5": "done"}));
    assert_eq!(
        state["swarm"]["orchestrators"].as_array().unwrap().len(),
        sw.orchestrators.len()
    );
    v.get_mut()
        .write_all(b"{\"seen\": [\"a5\"]}\n")
        .await
        .unwrap();
    let got = tokio::time::timeout(Duration::from_secs(5), l.next())
        .await
        .expect("seen in time");
    assert!(matches!(got, LinkEvent::Core(Event::Seen(ids)) if ids == ["a5"]));
    l.core = None; // the link held the only other handle
    if let Ok(c) = Arc::try_unwrap(c) {
        c.stop();
    }
}
