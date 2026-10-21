//! A filter chosen in one panel shows in every panel, over a real core socket (Python
//! tests/test_panel_filter.py::test_a_filter_chosen_in_one_panel_shows_in_every_panel): panel
//! A sends its filter, the core passes it on, the home view publishes it (here the test does
//! the home view's part with its own state writer), and panel B's state has it.

mod core_common;

use core_common::*;
use panorama::app::link::state::{Published, state_message};
use panorama::core::Event;
use panorama::proto::framing::encode;
use panorama::proto::messages::ToCore;
use panorama::proto::state::State;
use panorama_core::model::Swarm;
use panorama_core::view::{Filter, Marks};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

fn swarm() -> Swarm {
    serde_json::from_value(swarm_state()["state"]["swarm"].clone()).unwrap()
}

/// The filter a panel reads from a state line.
fn filter_of(msg: &Value) -> Option<Filter> {
    State::read(msg["state"].as_object().expect("a state"))
        .unwrap()
        .filter
}

#[tokio::test(flavor = "current_thread")]
async fn a_filter_chosen_in_one_panel_shows_in_every_panel() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(
        cfg(dir.path()),
        Panes {
            ours: vec!["w1:p92", "w1:p94"],
            ..Panes::default()
        },
    )
    .await;
    let (sock, _) = paths(dir.path());
    let (sw, marks, published) = (swarm(), Marks::new(), Published::default());
    let mut a = connect(&sock).await;
    hello(&mut a, json!({"pane_id": "w1:p92"})).await;
    let mut b = connect(&sock).await;
    hello(&mut b, json!({"pane_id": "w1:p94"})).await;
    lines(&mut a, 1).await; // welcome
    lines(&mut b, 1).await;
    c.publish(&state_message(
        &sw,
        &marks,
        &published,
        Some(Filter::All.name()),
    ));
    assert_eq!(filter_of(&lines(&mut a, 1).await[0]), Some(Filter::All));
    assert_eq!(filter_of(&lines(&mut b, 1).await[0]), Some(Filter::All));
    // f in panel A: its choice goes to the core
    let chose = ToCore::Filter(Filter::Needs.name().to_owned()).to_json(); // the panel's own message
    a.write_all(&encode(&chose).unwrap()).await.unwrap();
    let got = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
    let Ok(Some(Event::Filter(name))) = got else {
        panic!("no filter from panel A: {got:?}")
    };
    // the home view's part: a filter it knows is taken and published to every panel
    let chosen = Filter::from_name(&name).expect("a filter");
    c.publish(&state_message(&sw, &marks, &published, Some(chosen.name())));
    assert_eq!(filter_of(&lines(&mut b, 1).await[0]), Some(Filter::Needs));
    assert_eq!(filter_of(&lines(&mut a, 1).await[0]), Some(Filter::Needs));
    c.stop();
}

#[test]
fn the_state_carries_the_filter_and_the_version_says_so() {
    // test_panel_filter.py: protocol 1.3 (a 1.2 peer is compatible); the filter in the state,
    // left out when none is given
    assert_eq!(panorama::proto::PROTOCOL, "1.3");
    assert!(panorama::proto::compatible("1.2"));
    let (sw, marks, p) = (swarm(), Marks::new(), Published::default());
    assert_eq!(
        state_message(&sw, &marks, &p, Some("running"))["state"]["filter"],
        "running"
    );
    assert!(
        state_message(&sw, &marks, &p, None)["state"]
            .get("filter")
            .is_none()
    );
}
