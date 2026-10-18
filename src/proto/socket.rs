//! Where the core listens, and that it is ours (`docs/protocol/panel-v1.md`, "Transport").
//!
//! The path is the kit's (`internal::plugin_paths::panorama`, the same bytes as Python's
//! `protocol.socket_path`). A viewer talks only to a socket served by its own uid: anything
//! else is as good as no core.

use herdr_agent_kit::internal::plugin_paths::panorama::core_socket_path;
use herdr_agent_kit::plugin_env::PluginEnv;
use std::io;
use std::path::{Path, PathBuf};
use tokio::net::UnixStream;

/// The core's socket for this herdr server.
pub fn path(env: &PluginEnv) -> PathBuf {
    core_socket_path(env)
}

/// Why a connection was not made.
#[derive(Debug)]
pub enum ConnectError {
    /// No socket, or nobody listening.
    Io(io::Error),
    /// The socket is served by another user (`SO_PEERCRED` / `LOCAL_PEERCRED`).
    NotOurs { uid: u32 },
}

/// A connection to the core at `path`, checked to be served by `uid`.
pub async fn connect(path: &Path, uid: u32) -> Result<UnixStream, ConnectError> {
    let stream = UnixStream::connect(path).await.map_err(ConnectError::Io)?;
    let cred = stream.peer_cred().map_err(ConnectError::Io)?;
    if cred.uid() != uid {
        return Err(ConnectError::NotOurs { uid: cred.uid() });
    }
    Ok(stream)
}
