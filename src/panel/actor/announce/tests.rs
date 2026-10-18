//! The announcer against a scripted core on a real socket (as `tests/proto_client.rs`).

use super::*;
use crate::proto::client::{Config, Event};
use std::os::unix::fs::MetadataExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

/// A client welcomed by a scripted core; the core's side of the connection.
async fn connected() -> (
    Client,
    tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("core.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let uid = std::fs::metadata(dir.path()).unwrap().uid();
    let cfg = Config {
        path,
        uid,
        pane_id: None,
        tab_id: None,
        backoff: vec![Duration::from_millis(20)],
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = Client::spawn(cfg, tx);
    let (s, _) = listener.accept().await.unwrap();
    let (r, mut w) = s.into_split();
    let mut lines = BufReader::new(r).lines();
    lines.next_line().await.unwrap(); // the hello
    w.write_all(b"{\"welcome\":{}}\n").await.unwrap();
    assert_eq!(rx.recv().await, Some(Event::Connected(true)));
    std::mem::forget(w); // keep the connection open
    (client, lines, dir)
}

#[tokio::test(flavor = "current_thread")]
async fn an_echo_since_the_announcement_counts_an_older_one_does_not() {
    let (client, mut core, _dir) = connected().await;
    let (focus, rx) = watch::channel(Some("w1:p94".to_owned())); // an old state already named it
    let a = Announcer::new(client, rx);
    assert!(a.focusing("w1:p94").await);
    assert_eq!(
        core.next_line().await.unwrap().unwrap(),
        r#"{"focusing":{"pane_id":"w1:p94"}}"#
    );
    // nothing since: no echo, the whole wait
    assert!(!a.echoed("w1:p94", Duration::from_millis(30)).await);
    // the core echoes it (before the switcher even waits): it counts
    a.focusing("w1:p94").await;
    focus.send_replace(Some("w1:p94".into()));
    assert!(a.echoed("w1:p94", Duration::from_millis(30)).await);
    // another pane's state is no echo
    a.focusing("w1:p94").await;
    focus.send_replace(Some("w1:p2".into()));
    assert!(!a.echoed("w1:p94", Duration::from_millis(30)).await);
}

#[tokio::test(flavor = "current_thread")]
async fn a_state_before_the_announcement_is_no_echo() {
    // the reviewer's case (#77 M3): a state arrives between the switch's spawn and `focusing`
    let (client, mut core, _dir) = connected().await;
    let (focus, rx) = watch::channel(None);
    let a = Announcer::new(client, rx);
    focus.send_replace(Some("w1:p94".into()));
    assert!(a.focusing("w1:p94").await);
    core.next_line().await.unwrap();
    assert!(!a.echoed("w1:p94", Duration::from_millis(30)).await);
}
