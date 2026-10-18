//! The panel's protocol client against a scripted core on a real Unix socket (in a temp
//! dir): the hello, the welcome, states, queued seen marks, focusing and filter, shutdown,
//! a refused hello, and reconnecting after the core goes.

use panorama::proto::client::{Client, Config, Event};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

/// One accepted viewer: its lines in, ours out.
struct Peer {
    lines: tokio::io::Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
}

impl Peer {
    async fn accept(l: &UnixListener) -> Self {
        let (s, _) = timeout(WAIT, l.accept())
            .await
            .expect("no viewer came")
            .unwrap();
        let (r, write) = s.into_split();
        Self {
            lines: BufReader::new(r).lines(),
            write,
        }
    }

    async fn recv(&mut self) -> Value {
        let line = timeout(WAIT, self.lines.next_line())
            .await
            .expect("viewer silent")
            .unwrap()
            .expect("closed");
        serde_json::from_str(&line).unwrap()
    }

    async fn closed(&mut self) -> bool {
        matches!(
            timeout(WAIT, self.lines.next_line()).await,
            Ok(Ok(None)) | Ok(Err(_))
        )
    }

    async fn send(&mut self, v: Value) {
        let mut line = serde_json::to_vec(&v).unwrap();
        line.push(b'\n');
        self.write.write_all(&line).await.unwrap();
    }
}

fn uid_of(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().uid()
}

fn start(dir: &Path) -> (UnixListener, Client, mpsc::UnboundedReceiver<Event>) {
    let path = dir.join("core.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let cfg = Config {
        path,
        uid: uid_of(dir),
        pane_id: Some("w1:p9".into()),
        tab_id: Some("w1:t2".into()),
        backoff: vec![Duration::from_millis(20)],
    };
    let (tx, rx) = mpsc::unbounded_channel();
    (listener, Client::spawn(cfg, tx), rx)
}

async fn event(rx: &mut mpsc::UnboundedReceiver<Event>) -> Event {
    timeout(WAIT, rx.recv())
        .await
        .expect("no event")
        .expect("client gone")
}

fn swarm() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap()["swarm"].clone()
}

#[tokio::test(flavor = "current_thread")]
async fn hello_welcome_state_and_the_viewers_messages() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, client, mut rx) = start(dir.path());
    let mut core = Peer::accept(&listener).await;
    assert_eq!(
        core.recv().await,
        json!({"hello": {"protocol": "1.3", "role": "panel", "pane_id": "w1:p9", "tab_id": "w1:t2"}})
    );
    // before the welcome nothing goes out, and seen marks wait
    assert!(!client.focusing("w1:p3").await);
    client.seen(vec!["a1".into(), "a2".into()]);
    client.seen(vec!["a2".into()]);
    core.send(json!({"state": {"swarm": swarm()}})).await; // before the welcome: ignored
    core.send(json!({"welcome": {"protocol": "1.3"}})).await;
    assert_eq!(core.recv().await, json!({"seen": ["a1", "a2"]}));
    assert_eq!(event(&mut rx).await, Event::Connected(true));
    core.send(json!({"ping": 1})).await;
    core.send(json!({"state": {"swarm": swarm(), "filter": "running", "focused_pane": "w1:p3"}}))
        .await;
    match event(&mut rx).await {
        Event::State(st) => {
            assert_eq!(st.swarm.orchestrators.len(), 5);
            assert_eq!(st.focused_pane.as_deref(), Some("w1:p3"));
        }
        other => panic!("{other:?}"),
    }
    assert!(client.focusing("w1:p3").await);
    assert_eq!(core.recv().await, json!({"focusing": {"pane_id": "w1:p3"}}));
    assert!(client.filter("needs").await);
    assert_eq!(core.recv().await, json!({"filter": {"name": "needs"}}));
    client.seen(vec!["b3".into()]);
    assert_eq!(core.recv().await, json!({"seen": ["b3"]}));
    client.stop().await;
    assert_eq!(core.recv().await, json!({"bye": {}}));
    assert!(core.closed().await);
}

#[tokio::test(flavor = "current_thread")]
async fn a_state_that_does_not_read_is_dropped_and_the_next_one_comes_through() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, _client, mut rx) = start(dir.path());
    let mut core = Peer::accept(&listener).await;
    core.recv().await;
    core.send(json!({"welcome": {}})).await;
    assert_eq!(event(&mut rx).await, Event::Connected(true));
    let mut bad = swarm();
    bad["orchestrators"][0]["pane_id"] = json!(7);
    core.send(json!({"state": {"swarm": bad}})).await;
    core.send(json!({"state": {"swarm": swarm()}})).await;
    assert!(matches!(event(&mut rx).await, Event::State(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn the_client_reconnects_after_the_core_goes_and_flushes_queued_seen() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, client, mut rx) = start(dir.path());
    let mut core = Peer::accept(&listener).await;
    core.recv().await;
    core.send(json!({"welcome": {}})).await;
    assert_eq!(event(&mut rx).await, Event::Connected(true));
    drop(core); // the core crashed
    assert_eq!(event(&mut rx).await, Event::Connected(false));
    client.seen(vec!["c1".into()]);
    assert!(!client.filter("all").await);
    let mut core = Peer::accept(&listener).await; // the next core (same socket)
    assert_eq!(core.recv().await["hello"]["role"], "panel");
    core.send(json!({"welcome": {}})).await;
    assert_eq!(core.recv().await, json!({"seen": ["c1"]}));
    assert_eq!(event(&mut rx).await, Event::Connected(true));
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_stops_the_client_and_a_refused_hello_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, client, mut rx) = start(dir.path());
    let mut core = Peer::accept(&listener).await;
    core.recv().await;
    core.send(json!({"error": {"message": "protocol", "protocol": "2.0"}}))
        .await;
    assert_eq!(event(&mut rx).await, Event::Refused("protocol".into()));
    drop(core);
    let mut core = Peer::accept(&listener).await;
    core.recv().await;
    core.send(json!({"shutdown": {}})).await; // before the welcome: ignored
    core.send(json!({"welcome": {}})).await;
    assert_eq!(event(&mut rx).await, Event::Connected(true));
    core.send(json!({"shutdown": {}})).await;
    assert_eq!(event(&mut rx).await, Event::Shutdown);
    assert!(core.closed().await);
    assert!(!client.focusing("x").await); // stopped: nothing goes out
}

#[tokio::test(flavor = "current_thread")]
async fn a_socket_served_by_another_uid_is_no_core() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("core.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let cfg = Config {
        path,
        uid: uid_of(dir.path()).wrapping_add(1),
        pane_id: None,
        tab_id: None,
        backoff: vec![Duration::from_millis(20)],
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _client = Client::spawn(cfg, tx);
    // it connects (to look at the peer's uid) and leaves without a word, again and again
    for _ in 0..2 {
        let (s, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
        let mut lines = BufReader::new(s).lines();
        assert!(matches!(
            timeout(WAIT, lines.next_line()).await,
            Ok(Ok(None)) | Ok(Err(_))
        ));
    }
    assert!(rx.try_recv().is_err()); // never connected
}
