//! The viewer's connection to the core, as a task (Python `panel.PanelClient`).
//!
//! It connects, says hello, and reports what the core says as [`Event`]s; when the
//! connection ends it reconnects after a backoff (0.25, 0.5, 1, 2, then every 5 s; back to
//! the start after a session that got its welcome). `seen` marks made without a core are
//! queued and sent after the next welcome. The panel app decides what a long absence means
//! (it exits after 10 minutes without a core).

mod backoff;
mod connection;
mod session;

pub use backoff::{BACKOFF, Backoff};

use super::messages::ToCore;
use super::state::State;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// What the client reports.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// A core welcomed us (`true`), or the connection to it ended (`false`).
    Connected(bool),
    /// A snapshot from the core.
    State(Box<State>),
    /// The core refused our hello (an incompatible protocol): its message. It stands until
    /// the next `Connected(true)` (Python clears `refused` on the welcome).
    Refused(String),
    /// The core says the panels go: the viewer exits. The client has stopped.
    Shutdown,
}

/// Where and as whom to connect.
#[derive(Clone, Debug)]
pub struct Config {
    /// The core's socket.
    pub path: PathBuf,
    /// Our uid: a socket served by anyone else is no core.
    pub uid: u32,
    /// This panel's pane and tab, claimed in the hello.
    pub pane_id: Option<String>,
    pub tab_id: Option<String>,
    /// The reconnect waits ([`BACKOFF`]; empty: that).
    pub backoff: Vec<Duration>,
}

pub(crate) enum Command {
    /// Send now if a core is connected; the answer says whether it went out.
    Send(ToCore, oneshot::Sender<bool>),
    /// Seen marks: sent now, or queued until the next welcome.
    Seen(Vec<String>),
    /// Say bye and stop.
    Stop(oneshot::Sender<()>),
}

/// The handle the panel holds. Dropping the last clone stops the client.
#[derive(Clone)]
pub struct Client {
    tx: mpsc::UnboundedSender<Command>,
}

impl Client {
    /// Starts the client on the current tokio runtime; its events go to `events`.
    pub fn spawn(cfg: Config, events: mpsc::UnboundedSender<Event>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(session::run(cfg, rx, events));
        Self { tx }
    }

    async fn send(&self, msg: ToCore) -> bool {
        let (reply, answer) = oneshot::channel();
        self.tx.send(Command::Send(msg, reply)).is_ok() && answer.await.unwrap_or(false)
    }

    /// Announces a switch's focus (1.2). False without a core.
    pub async fn focusing(&self, pane_id: &str) -> bool {
        self.send(ToCore::Focusing(pane_id.to_owned())).await
    }

    /// Tells the core the filter chosen here (1.3). False without a core: the choice stays
    /// this panel's until the next core's state.
    pub async fn filter(&self, name: &str) -> bool {
        self.send(ToCore::Filter(name.to_owned())).await
    }

    /// Reports seen agents; queued while there is no core.
    pub fn seen(&self, ids: Vec<String>) {
        let _ = self.tx.send(Command::Seen(ids));
    }

    /// Says bye to the core (if any) and stops the client.
    pub async fn stop(&self) {
        let (done, wait) = oneshot::channel();
        if self.tx.send(Command::Stop(done)).is_ok() {
            let _ = wait.await;
        }
    }
}
