//! The core's server side against real sockets: Python's tests/test_core.py server cases,
//! ported (the core's own process plays every viewer).

mod core_common;

use core_common::*;
use panorama::core::{Event, Limits, NotStarted, Role, events, start};
use serde_json::json;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test(flavor = "current_thread")]
async fn one_core_per_session_and_its_socket_is_private() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _rx) = up(cfg(dir.path()), Panes::default()).await;
    assert!(matches!(
        start_within(cfg(dir.path()), 5),
        Err(NotStarted::Locked)
    ));
    let (sock, _) = paths(dir.path());
    assert_eq!(
        std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(sock.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    first.stop();
    assert!(!sock.exists());
    let (again, _rx3) = up(cfg(dir.path()), Panes::default()).await; // the lock went with it
    again.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_dead_cores_socket_is_replaced_under_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (sock, _) = paths(dir.path());
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(sock.parent().unwrap())
        .unwrap();
    drop(std::os::unix::net::UnixListener::bind(&sock).unwrap()); // left behind: nobody listens
    assert!(sock.exists());
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await;
    let mut v = connect(&sock).await;
    hello(&mut v, json!({"pane_id": "w1:p9"})).await;
    assert!(lines(&mut v, 1).await[0].get("welcome").is_some());
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn another_users_connection_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = cfg(dir.path());
    c.peer_uid = uid() + 1; // every peer here is "another user"
    let (c, mut rx) = up(c, Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    let _ = v
        .write_all(&line(json!({"hello": {"protocol": "1.3"}})))
        .await;
    assert!(closed(&mut v).await); // closed without a word
    assert!(c.connected().is_empty());
    assert!(matches!(rx.recv().await, Some(Event::Note(n)) if n.contains("another user")));
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn hello_welcome_state_seen_and_an_old_major_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(
        cfg(dir.path()),
        Panes {
            ours: vec!["w1:p9"],
            ..Panes::default()
        },
    )
    .await;
    c.publish(&swarm_state());
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    hello(
        &mut v,
        json!({"role": "panel", "pane_id": "w1:p9", "tab_id": "w1:t3"}),
    )
    .await;
    let got = lines(&mut v, 2).await;
    assert_eq!(got[0], json!({"welcome": {"protocol": "1.3"}}));
    assert_eq!(got[1]["state"]["unseen"], json!({"a5": "done"}));
    assert_eq!(got[1]["state"]["visible_tabs"], json!(["w1:t3"]));
    assert!(
        wait(|| c
            .connected()
            .first()
            .is_some_and(|v| v.pane_id.as_deref() == Some("w1:p9")))
        .await
    ); // verified
    assert_eq!(c.connected()[0].tab_id.as_deref(), Some("w1:t3"));
    v.write_all(&line(json!({"seen": ["a5", 7]})))
        .await
        .unwrap();
    assert!(matches!(rx.recv().await, Some(Event::Seen(ids)) if ids == ["a5"]));
    let mut old = connect(&sock).await;
    old.write_all(&line(json!({"hello": {"protocol": "2.0"}})))
        .await
        .unwrap();
    assert_eq!(
        lines(&mut old, 1).await[0],
        json!({"error": {"message": "protocol", "protocol": "1.3"}})
    );
    assert!(closed(&mut old).await);
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_hellos_protocol_reads_as_pythons_str_of_it() {
    // Python: major(str(version)): a number 1 or 1.3 is compatible, a missing one is not
    let dir = tempfile::tempdir().unwrap();
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    for (p, ok) in [
        (json!(1), true),
        (json!(1.3), true),
        (json!("1"), true),
        (json!(null), false),
        (json!("x"), false),
    ] {
        let mut v = connect(&sock).await;
        v.write_all(&line(json!({"hello": {"protocol": p}})))
            .await
            .unwrap();
        let first = lines(&mut v, 1).await.remove(0);
        assert_eq!(first.get("welcome").is_some(), ok, "{p}");
    }
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn an_unverified_pane_claim_is_not_trusted_and_the_claim_is_checked_against_the_peer() {
    // test_an_unverified_pane_claim_is_not_trusted, test_a_pane_claim_is_checked_against_the_peers_own_process
    let dir = tempfile::tempdir().unwrap();
    let panes = Panes::default();
    let (c, _rx) = up(cfg(dir.path()), panes.clone()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    hello(&mut v, json!({"pane_id": "w9:p1"})).await;
    lines(&mut v, 1).await;
    assert!(wait(|| !c.connected().is_empty()).await);
    assert_eq!(c.connected()[0].pane_id, None);
    assert_eq!(
        *panes.asked.lock().unwrap(),
        [("w9:p1".to_owned(), std::process::id() as i32)]
    );
    assert!(c.verified_panes().is_empty());
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_stuck_viewer_never_stalls_the_core() {
    let dir = tempfile::tempdir().unwrap();
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut stuck = connect(&sock).await;
    hello(&mut stuck, json!({})).await;
    assert!(wait(|| !c.connected().is_empty()).await);
    let big = swarm_state();
    let t0 = std::time::Instant::now();
    for _ in 0..300 {
        c.publish(&big); // far more than the socket buffer holds
    }
    assert!(t0.elapsed() < Duration::from_secs(2));
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn messages_before_hello_close_the_connection() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    v.write_all(&line(json!({"seen": ["a1"]}))).await.unwrap();
    assert!(closed(&mut v).await);
    assert!(rx.try_recv().is_err()); // nothing seen
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn one_read_reports_seen_once() {
    // test_the_line_reader_is_linear_and_one_read_reports_seen_once (the reader's half is
    // proto::framing's)
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    let mut all = line(json!({"hello": {"protocol": "1.3"}}));
    all.extend(line(json!({"seen": ["a1"]})));
    all.extend(line(json!({"seen": ["a2", "a1"]})));
    v.write_all(&all).await.unwrap();
    assert!(matches!(rx.recv().await, Some(Event::Seen(ids)) if ids == ["a1", "a2"]));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(rx.try_recv().is_err());
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn an_unsafe_socket_directory_is_refused() {
    // test_an_unsafe_socket_directory_is_refused[symlink, group-writable, not-ours] and
    // test_the_tmp_fallbacks_parent_must_be_safe_too
    for bad in [
        "symlink",
        "group-writable",
        "not-ours",
        "parent-world-writable",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (sock, _) = paths(dir.path());
        let real = dir.path().join("elsewhere");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&real)
            .unwrap();
        let mut c = cfg(dir.path());
        match bad {
            "symlink" => std::os::unix::fs::symlink(&real, sock.parent().unwrap()).unwrap(),
            "group-writable" => {
                std::fs::create_dir(sock.parent().unwrap()).unwrap();
                std::fs::set_permissions(
                    sock.parent().unwrap(),
                    std::fs::Permissions::from_mode(0o770),
                )
                .unwrap();
            }
            "not-ours" => {
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(sock.parent().unwrap())
                    .unwrap();
                c.uid = uid() + 1;
            }
            _ => std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777))
                .unwrap(),
        }
        let (tx, _rx) = events();
        assert!(
            matches!(
                start(c, Panes::default(), tx).await,
                Err(panorama::core::NotStarted::Dir(_))
            ),
            "{bad}"
        );
        assert!(!sock.exists() && !real.join("core.sock").exists(), "{bad}");
    }
    // like /tmp, but ours: fine
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o1777)).unwrap();
    let (c, _rx) = up(cfg(dir.path()), Panes::default()).await;
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_connection_without_hello_is_closed_at_the_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        hello: Duration::from_millis(300),
        ..Limits::default()
    };
    let (c, _rx) = up(quick(dir.path(), limits), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    v.write_all(b"{\"hel").await.unwrap(); // a start, never finished
    let t0 = std::time::Instant::now();
    assert!(closed(&mut v).await && t0.elapsed() < Duration::from_secs(2));
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn idle_connections_are_capped() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        max_viewers: 8,
        ..Limits::default()
    };
    let (c, mut rx) = up(quick(dir.path(), limits), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut idle = vec![];
    for _ in 0..30 {
        if let Ok(s) = UnixStream::connect(&sock).await {
            idle.push(s);
        }
    }
    // the ones over the cap are closed at once, and said once
    let mut refused = 0;
    for s in idle.iter_mut() {
        let mut b = [0u8; 1];
        if matches!(
            tokio::time::timeout(Duration::from_millis(50), s.read(&mut b)).await,
            Ok(Ok(0))
        ) {
            refused += 1;
        }
    }
    assert_eq!(refused, idle.len() - 8);
    let notes: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|e| {
            if let Event::Note(n) = e {
                Some(n)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        notes
            .iter()
            .filter(|n| n.contains("refusing connections"))
            .count(),
        1
    );
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn a_viewer_that_never_reads_is_dropped_after_the_send_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        send: Duration::from_millis(300),
        ..Limits::default()
    };
    let (c, _rx) = up(quick(dir.path(), limits), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut stuck = connect(&sock).await;
    hello(&mut stuck, json!({})).await;
    assert!(wait(|| !c.connected().is_empty()).await);
    let big = swarm_state();
    let end = std::time::Instant::now() + Duration::from_secs(5);
    while !c.connected().is_empty() && std::time::Instant::now() < end {
        c.publish(&big);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(c.connected().is_empty());
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn focusing_counts_only_from_a_verified_panel_and_filters_only_from_panels() {
    // test_panel_sync.py::test_the_core_announces_a_focus_only_from_a_verified_panel;
    // test_panel_filter.py::test_the_core_passes_a_panels_filter_on
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(
        cfg(dir.path()),
        Panes {
            ours: vec!["w1:p92"],
            ..Panes::default()
        },
    )
    .await;
    let (sock, _) = paths(dir.path());
    let mut stranger = connect(&sock).await;
    hello(&mut stranger, json!({"pane_id": "w1:p50"})).await;
    lines(&mut stranger, 1).await;
    stranger
        .write_all(&line(json!({"focusing": {"pane_id": "w1:p94"}})))
        .await
        .unwrap();
    let mut ours = connect(&sock).await;
    hello(&mut ours, json!({"pane_id": "w1:p92"})).await;
    lines(&mut ours, 1).await;
    ours.write_all(&line(json!({"focusing": {"pane_id": ""}})))
        .await
        .unwrap(); // no pane: nothing to announce
    ours.write_all(&line(json!({"focusing": {"pane_id": "w1:p94"}})))
        .await
        .unwrap();
    ours.write_all(&line(json!({"filter": {"name": "needs"}})))
        .await
        .unwrap();
    ours.write_all(&line(json!({"filter": "junk"})))
        .await
        .unwrap(); // not an object: ignored
    assert!(matches!(rx.recv().await, Some(Event::Focusing(p)) if p == "w1:p94"));
    assert!(matches!(rx.recv().await, Some(Event::Filter(f)) if f == "needs"));
    let mut key = connect(&sock).await;
    hello(&mut key, json!({"role": "key"})).await;
    lines(&mut key, 1).await;
    key.write_all(&line(json!({"filter": {"name": "all"}})))
        .await
        .unwrap(); // a key chooses no filter
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(rx.try_recv().is_err());
    assert!(c.connected().iter().any(|v| v.role == Role::Key));
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn the_open_key_is_answered_and_gets_no_state() {
    let dir = tempfile::tempdir().unwrap();
    let (c, mut rx) = up(cfg(dir.path()), Panes::default()).await;
    c.publish(&swarm_state());
    let (sock, _) = paths(dir.path());
    let mut key = connect(&sock).await;
    hello(&mut key, json!({"role": "key"})).await;
    assert!(wait(|| !c.connected().is_empty()).await);
    c.publish(&swarm_state()); // a key connection gets no state, before or after its hello
    key.write_all(&line(json!({"open": {}}))).await.unwrap();
    let Some(Event::Open { reply, .. }) = rx.recv().await else {
        panic!("no open request")
    };
    reply.send("panel".into()).unwrap();
    assert_eq!(
        lines(&mut key, 2).await,
        [
            json!({"welcome": {"protocol": "1.3"}}),
            json!({"opened": {"result": "panel"}})
        ]
    );
    c.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_reaches_every_panel_and_pings_follow_states() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        ping: Duration::from_millis(100),
        ..Limits::default()
    };
    let (c, _rx) = up(quick(dir.path(), limits), Panes::default()).await;
    let (sock, _) = paths(dir.path());
    let mut v = connect(&sock).await;
    hello(&mut v, json!({})).await;
    let got = lines(&mut v, 2).await;
    assert_eq!(got[1], json!({"ping": 1}));
    c.shutdown(Duration::from_secs(1)).await;
    let mut rest = lines(&mut v, 1).await;
    while rest.last() != Some(&json!({"shutdown": {}})) {
        rest = lines(&mut v, 1).await; // pings queued before it
    }
    c.publish(&swarm_state()); // nothing after shutdown
    let mut b = [0u8; 64];
    let after = tokio::time::timeout(Duration::from_millis(300), v.read(&mut b)).await;
    assert!(after.is_err(), "something came after shutdown: {after:?}");
    c.stop();
    assert!(closed(&mut v).await);
}
