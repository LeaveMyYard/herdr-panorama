//! New observer process: initialization and read methods only. No start, resume,
//! subscription, model request or authentication operation is permitted here.
use super::Options;
use serde_json::{Value, json};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const MAX_FRAME: u64 = 2 * 1024 * 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_MESSAGES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unavailable,
    Unsupported,
    Invalid,
    Limit,
}
pub(super) type Result<T> = std::result::Result<T, Error>;

pub(super) struct Client {
    pub child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
    bytes: usize,
    messages: usize,
}
impl Client {
    pub fn spawn(options: &Options) -> Result<Self> {
        let mut command = Command::new(&options.executable);
        command
            .args(["app-server", "--listen", "stdio://"])
            .env("CODEX_HOME", &options.codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .process_group(0);
        // No observer subprocess should inherit a live pane's mutation routing.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("HERDR_") {
                command.env_remove(key);
            }
        }
        let mut child = command.spawn().map_err(|_| Error::Unavailable)?;
        let input = child.stdin.take().ok_or(Error::Unavailable)?;
        let output = BufReader::new(child.stdout.take().ok_or(Error::Unavailable)?);
        Ok(Self {
            child,
            input,
            output,
            next_id: 0,
            bytes: 0,
            messages: 0,
        })
    }
    async fn send(&mut self, value: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value).map_err(|_| Error::Invalid)?;
        bytes.push(b'\n');
        self.input
            .write_all(&bytes)
            .await
            .map_err(|_| Error::Unavailable)?;
        self.input.flush().await.map_err(|_| Error::Unavailable)
    }
    pub async fn initialize(&mut self) -> Result<()> {
        self.call("initialize", json!({"clientInfo": {"name":"herdr-panorama-observer","version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"experimentalApi":true,"explicitGatewayOauth":true}})).await?;
        self.send(json!({"method":"initialized"})).await
    }
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        if !matches!(
            method,
            "initialize" | "thread/read" | "thread/turns/list" | "thread/items/list"
        ) {
            return Err(Error::Invalid);
        }
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        loop {
            if self.bytes >= MAX_BYTES || self.messages >= MAX_MESSAGES {
                return Err(Error::Limit);
            }
            let mut line = Vec::new();
            (&mut self.output)
                .take(MAX_FRAME + 1)
                .read_until(b'\n', &mut line)
                .await
                .map_err(|_| Error::Unavailable)?;
            self.bytes += line.len();
            self.messages += 1;
            if line.is_empty() {
                return Err(Error::Unavailable);
            }
            if line.len() > MAX_FRAME as usize || self.bytes > MAX_BYTES {
                return Err(Error::Limit);
            }
            let value: Value = serde_json::from_slice(&line).map_err(|_| Error::Invalid)?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(
                    if error.get("code").and_then(Value::as_i64) == Some(-32601) {
                        Error::Unsupported
                    } else {
                        Error::Unavailable
                    },
                );
            }
            return value.get("result").cloned().ok_or(Error::Invalid);
        }
    }
}

impl Client {
    fn kill_group(&self) {
        if let Some(pid) = self
            .child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
    pub async fn stop(&mut self) {
        // Includes older npm launchers that spawn the native Codex binary.
        self.kill_group();
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.kill_group();
    }
}
