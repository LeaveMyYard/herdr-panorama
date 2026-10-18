//! Starting the core (Python `Core._start`) and its two background tasks: accepting
//! connections and the ping.

use super::viewer::Viewer;
use super::{Config, Core, Event, Shared, Verify, conn};
use herdr_agent_kit::plugin_env::lock::LockFile;
use herdr_agent_kit::plugin_env::private_dir::make_private_dir;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

/// Why the core did not start.
#[derive(Debug)]
pub enum NotStarted {
    /// The socket's directory cannot be made, or is not private.
    Dir(String),
    /// Another core holds the lock (the usual case: a second panorama).
    Locked,
    /// A core answers on the socket without holding the lock.
    Answering,
    /// The lock or the socket failed.
    Io(std::io::Error),
}

impl std::fmt::Display for NotStarted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dir(why) => write!(f, "not starting: {why}"),
            Self::Locked => f.write_str("another core is running"),
            Self::Answering => {
                f.write_str("a core answers on the socket without its lock: not starting")
            }
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Starts the core: the private directory, the lock, a dead core's socket cleared, the socket
/// bound 0600; then it accepts viewers and pings them. What viewers say goes to `events`.
pub async fn start(
    cfg: Config,
    verify: impl Verify,
    events: mpsc::UnboundedSender<Event>,
) -> Result<Core, NotStarted> {
    let dir = cfg.sock.parent().unwrap_or(std::path::Path::new("."));
    make_private_dir(dir, cfg.uid).map_err(|e| NotStarted::Dir(e.to_string()))?;
    let lock = LockFile::private(&cfg.lock)
        .try_exclusive()
        .map_err(NotStarted::Io)?
        .ok_or(NotStarted::Locked)?;
    if cfg.sock.exists() {
        let probe =
            tokio::time::timeout(Duration::from_millis(500), UnixStream::connect(&cfg.sock)).await;
        if matches!(probe, Ok(Ok(_))) {
            return Err(NotStarted::Answering);
        }
        let _ = std::fs::remove_file(&cfg.sock); // a dead core's leftover, removed under our lock
    }
    let listener = UnixListener::bind(&cfg.sock).map_err(NotStarted::Io)?;
    // the directory is private (0700, ours), so the moment before the chmod exposes nothing
    std::fs::set_permissions(&cfg.sock, std::fs::Permissions::from_mode(0o600))
        .map_err(NotStarted::Io)?;
    let shared = Arc::new(Shared {
        limits: cfg.limits,
        uid: cfg.peer_uid,
        viewers: Mutex::new(vec![]),
        last: Mutex::new(None),
        events,
        refusing: false.into(),
    });
    let verify = Arc::new(verify);
    let tasks = vec![
        tokio::spawn(accept(listener, shared.clone(), verify)),
        tokio::spawn(ping(shared.clone())),
    ];
    Ok(Core {
        shared,
        sock: cfg.sock,
        tasks,
        _lock: lock,
    })
}

async fn accept(listener: UnixListener, shared: Arc<Shared>, verify: Arc<impl Verify>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let cred = stream.peer_cred().ok();
        if cred.map(|c| c.uid()) != Some(shared.uid) {
            let _ = shared
                .events
                .send(Event::Note("refused a connection from another user".into()));
            continue;
        }
        let v = {
            let mut viewers = shared.viewers.lock().unwrap();
            (viewers.len() < shared.limits.max_viewers).then(|| {
                let v = Arc::new(Viewer::new(
                    cred.and_then(|c| c.pid()),
                    shared.limits.outbox,
                ));
                viewers.push(v.clone());
                v
            })
        };
        let Some(v) = v else {
            // once per full spell, not once per connection
            if !shared.refusing.swap(true, Ordering::SeqCst) {
                let n = shared.limits.max_viewers;
                let _ = shared.events.send(Event::Note(format!(
                    "refusing connections: {n} open already"
                )));
            }
            continue;
        };
        shared.refusing.store(false, Ordering::SeqCst);
        tokio::spawn(conn::serve(stream, v, shared.clone(), verify.clone()));
    }
}

async fn ping(shared: Arc<Shared>) {
    let mut n = 0u64;
    loop {
        tokio::time::sleep(shared.limits.ping).await;
        n += 1;
        let line =
            Arc::new(crate::proto::framing::encode(&serde_json::json!({"ping": n})).unwrap());
        for v in shared.viewers.lock().unwrap().iter() {
            if v.wants_state() {
                v.send(line.clone());
            }
        }
    }
}
