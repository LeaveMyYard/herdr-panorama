//! The core's server side under pressure and at its edges (the review of #85): a flooding
//! viewer, a live core already answering, a slow pane check, hellos after shutdown or a
//! second incompatible one, and a stuck writer when the core goes.

mod core_common;

use core_common::*;
use panorama::core::{EVENTS, Event, Limits, NotStarted};
use serde_json::json;
use std::os::unix::fs::DirBuilderExt;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

#[tokio::test(flavor = "current_thread")]
async fn a_flooding_viewer_is_slowed_down_and_the_others_are_still_served() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut flood = connect(&sock).await;
    hello(&mut flood, json!({})).await;
    lines(&mut flood, 1).await;
    let flooding = tokio::spawn(async move {
        let chunk = line(json!({"filter": {"name": "x"}})).repeat(1000);
        for _ in 0..200 {
            if flood.write_all(&chunk).await.is_err() {
                break;
            }
        }
        flood
    });
    tokio::time::sleep(Duration::from_millis(1500)).await; // the app reads nothing meanwhile
    assert!(rx.len() <= EVENTS, "{} events queued", rx.len());
    assert!(!flooding.is_finished(), "the flood went through unread");
    let mut other = connect(&sock).await;
    hello(&mut other, json!({})).await;
    assert!(lines(&mut other, 1).await[0].get("welcome").is_some());
    c.publish(&swarm_state());
    assert!(lines(&mut other, 1).await[0].get("state").is_some());
    // once the app reads again the flood moves on
    let mut n = 0;
    let end = Instant::now() + Duration::from_secs(5);
    while n <= 2 * EVENTS && Instant::now() < end {
        if let Ok(Some(Event::Filter(_))) =
            tokio::time::timeout(Duration::from_secs(1), rx.recv()).await
        {
            n += 1;
        }
    }
    assert!(n > 2 * EVENTS);
    // the core stops while the flood's reader waits on a full channel (the app still holds it):
    // that reader lets go too, so the flood's socket closes
    tokio::time::sleep(Duration::from_millis(300)).await;
    c.stop();
    let ended = tokio::time::timeout(Duration::from_secs(3), flooding).await;
    assert!(
        ended.is_ok(),
        "the flood's connection stayed open after the core stopped"
    );
    drop(rx);
}

#[tokio::test(flavor = "current_thread")]
async fn a_core_answering_on_the_socket_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (sock, _) = paths(dir.path());
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(sock.parent().unwrap())
        .unwrap();
    let live = std::os::unix::net::UnixListener::bind(&sock).unwrap(); // answers, holds no lock
    assert!(matches!(
        start_within(cfg(dir.path()), 5),
        Err(NotStarted::Answering)
    ));
    assert!(sock.exists(), "a live core's socket was removed");
    let _peer = std::os::unix::net::UnixStream::connect(&sock).unwrap();
    assert!(live.accept().is_ok(), "the live core still answers");
}

#[tokio::test(flavor = "current_thread")]
async fn a_pane_check_slower_than_the_hello_deadline_reads_as_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        hello: Duration::from_millis(300),
        ..Limits::default()
    };
    let panes = Panes {
        ours: vec!["w1:p9"],
        slow: Duration::from_secs(3),
        ..Panes::default()
    };
    let (c, _rx) = up(quick(dir.path(), limits), panes).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    let t0 = Instant::now();
    hello(&mut v, json!({"pane_id": "w1:p9"})).await;
    assert!(lines(&mut v, 1).await[0].get("welcome").is_some());
    assert!(
        t0.elapsed() < Duration::from_secs(1),
        "welcomed after {:?}",
        t0.elapsed()
    );
    assert!(c.verified_panes().is_empty());
    assert_eq!(c.connected().len(), 1);
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_panel_that_says_hello_after_shutdown_is_told_to_go_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        ping: Duration::from_millis(100),
        ..Limits::default()
    };
    let (c, _rx) = up(quick(dir.path(), limits), Panes::default()).await;
    c.publish(&swarm_state());
    c.shutdown(Duration::from_millis(100)).await;
    let (sock, _) = paths(dir.path());
    let mut late = connect(&sock).await;
    hello(&mut late, json!({})).await;
    let got = lines(&mut late, 2).await;
    assert!(got[0].get("welcome").is_some());
    assert_eq!(got[1], json!({"shutdown": {}}), "no state before it");
    c.publish(&swarm_state());
    let mut b = [0u8; 64];
    let after = tokio::time::timeout(
        Duration::from_millis(400),
        tokio::io::AsyncReadExt::read(&mut late, &mut b),
    )
    .await;
    assert!(after.is_err(), "something came after shutdown: {after:?}");
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_incompatible_hello_is_answered_then_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    hello(&mut v, json!({})).await;
    assert!(lines(&mut v, 1).await[0].get("welcome").is_some());
    v.write_all(&line(json!({"hello": {"protocol": "2.0"}})))
        .await
        .unwrap();
    let got = lines(&mut v, 1).await;
    assert_eq!(got[0]["error"]["message"], "protocol");
    assert!(closed(&mut v).await);
    assert!(wait(|| c.connected().is_empty()).await);
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_stuck_writer_lets_go_of_the_socket_when_the_core_stops() {
    let dir = tempfile::tempdir().unwrap();
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await; // the send timeout: 10 s
    let (sock, _) = paths(dir.path());
    let mut stuck = connect(&sock).await;
    hello(&mut stuck, json!({})).await;
    assert!(wait(|| !c.connected().is_empty()).await);
    let big = swarm_state();
    for _ in 0..200 {
        c.publish(&big); // the socket fills; the writer waits inside a write
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    c.stop();
    tokio::time::sleep(Duration::from_millis(200)).await;
    peer_closed_unread(&stuck);
    // and, on every system: what was sent is there to read, then the end
    let mut rest = Vec::new();
    let read = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::io::AsyncReadExt::read_to_end(&mut stuck, &mut rest),
    )
    .await;
    assert!(matches!(read, Ok(Ok(_))), "no end of the stream: {read:?}");
}

/// Linux: the core closed the socket while its unread data still waits (`POLLRDHUP` says so
/// without reading). Other systems have no such flag; the read to the end above stands in.
#[cfg(target_os = "linux")]
fn peer_closed_unread(s: &tokio::net::UnixStream) {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut fds = [PollFd::new(s, PollFlags::RDHUP)];
    let second = Timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };
    assert_eq!(
        poll(&mut fds, Some(&second)).unwrap(),
        1,
        "the core still holds the socket"
    );
    assert!(fds[0].revents().contains(PollFlags::RDHUP));
}

#[cfg(not(target_os = "linux"))]
fn peer_closed_unread(_: &tokio::net::UnixStream) {}

#[tokio::test(flavor = "current_thread")]
async fn a_long_seen_line_is_cut_and_never_stalls_the_other_viewers() {
    // the review of #85: 80k distinct ids froze the core for 4.9 s with a quadratic dedup
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    hello(&mut v, json!({})).await;
    lines(&mut v, 1).await;
    let ids: Vec<String> = (0..50_000).map(|i| format!("a{i}")).collect();
    v.write_all(&line(json!({ "seen": ids }))).await.unwrap();
    let mut other = connect(&sock).await;
    hello(&mut other, json!({})).await;
    assert!(lines(&mut other, 1).await[0].get("welcome").is_some()); // within lines()' 5 s
    let got = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(Event::Seen(ids)) = rx.recv().await {
                return ids;
            }
        }
    })
    .await
    .expect("the seen event");
    assert_eq!(got.len(), panorama::core::MAX_SEEN);
    assert_eq!(got[0], "a0");
    assert_eq!(
        got[got.len() - 1],
        format!("a{}", panorama::core::MAX_SEEN - 1)
    );
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn an_open_request_the_app_never_answers_ends_with_the_core() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut key = connect(&sock).await;
    hello(&mut key, json!({"role": "key"})).await;
    lines(&mut key, 1).await;
    key.write_all(&line(json!({"open": {}}))).await.unwrap();
    let reply = loop {
        if let Some(Event::Open { reply, .. }) = rx.recv().await {
            break reply; // held, never answered
        }
    };
    c.stop();
    assert!(
        wait(|| reply.is_closed()).await,
        "the connection still waits for the answer after the core stopped"
    );
}
