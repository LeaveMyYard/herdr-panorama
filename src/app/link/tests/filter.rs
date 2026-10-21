//! The home view's own filter goes to every panel (Python `_set_filter` publishes), and a
//! panel's filter taken here goes out to the others.

use super::panels::{core, viewer};
use crate::app::model::App;
use crate::ui::input::Key;
use herdr_agent_kit::herdr_api::fake::FakeHerdr;
use panorama_core::view::{Filter, Marks};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt;

async fn next_state(v: &mut tokio::io::BufReader<tokio::net::UnixStream>) -> Value {
    loop {
        let mut l = String::new();
        let n = tokio::time::timeout(Duration::from_secs(5), v.read_line(&mut l))
            .await
            .unwrap()
            .unwrap();
        assert!(n > 0, "closed");
        let m: Value = serde_json::from_str(&l).unwrap();
        if m.get("state").is_some() {
            return m["state"].clone();
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_home_views_own_filter_and_a_panels_go_out_to_every_panel() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let c = core(dir.path()).await;
    let mut v = viewer(dir.path()).await;
    let mut l = super::link(&fake, true, dir.path());
    l.core = Some(c);
    let t0 = Instant::now() - Duration::from_secs(5);
    let mut app = App::new(Some("w1:p1".into()), true, t0);
    app.apply(Default::default(), Marks::new(), t0);
    app.armed_at = t0;
    // the home view's own f: the view changed, so the panels hear it
    let before = app.view.filter;
    app.on_key(Key::Filter, Instant::now());
    assert_ne!(app.view.filter, before);
    assert!(l.filter_changed(app.view.filter));
    l.publish(
        &app.graph.swarm,
        &app.graph.view.unseen,
        app.graph.view.filter,
    );
    assert_eq!(next_state(&mut v).await["filter"], app.view.filter.name());
    assert!(!l.filter_changed(app.view.filter), "said once");
    // a panel's: taken here, then published
    assert!(app.filter_from_core("needs", Instant::now()).is_some());
    assert!(l.filter_changed(app.view.filter));
    l.publish(
        &app.graph.swarm,
        &app.graph.view.unseen,
        app.graph.view.filter,
    );
    assert_eq!(next_state(&mut v).await["filter"], Filter::Needs.name());
}
