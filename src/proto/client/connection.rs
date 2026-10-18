//! One connection to the core, until it ends: connect (to a socket served by our uid), say
//! hello, then read the core's messages and carry out the panel's commands.

use super::session::{End, Task};
use super::{Command, Event};
use crate::proto::framing::{LineReader, encode};
use crate::proto::messages::{FromCore, ToCore};
use crate::proto::socket;
use crate::proto::state::State;
use serde_json::{Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;

/// A read buffer's size.
const READ: usize = 65536;

impl Task {
    /// One connection, until it ends.
    pub(super) async fn session(&mut self) -> End {
        let Ok(stream) = socket::connect(&self.cfg.path, self.cfg.uid).await else {
            return End::Lost { welcomed: false };
        };
        let (mut read, mut write) = stream.into_split();
        let hello = ToCore::Hello {
            pane_id: self.cfg.pane_id.clone(),
            tab_id: self.cfg.tab_id.clone(),
        };
        if put(&mut write, &hello).await.is_err() {
            return End::Lost { welcomed: false };
        }
        let mut welcomed = false;
        let mut reader = LineReader::default();
        let mut buf = vec![0u8; READ];
        loop {
            let end = tokio::select! {
                got = read.read(&mut buf) => match got {
                    Ok(0) | Err(_) => Some(End::Lost { welcomed }),
                    Ok(n) => match reader.feed(&buf[..n]) {
                        Ok(msgs) => self.on_core(msgs, &mut write, &mut welcomed).await,
                        Err(_) => Some(End::Lost { welcomed }), // a line over the limit
                    },
                },
                cmd = self.rx.recv() => self.on_command(cmd, &mut write).await,
            };
            if let Some(end) = end {
                return end;
            }
        }
    }

    /// The core's messages, in order; `Some` when the connection ends.
    async fn on_core(
        &mut self,
        msgs: Vec<Map<String, Value>>,
        write: &mut OwnedWriteHalf,
        welcomed: &mut bool,
    ) -> Option<End> {
        for msg in msgs {
            match FromCore::read(msg) {
                FromCore::Welcome => {
                    *welcomed = true;
                    let pending = std::mem::take(&mut self.pending_seen);
                    if !pending.is_empty()
                        && put(write, &ToCore::Seen(pending.clone())).await.is_err()
                    {
                        self.queue_seen(pending);
                        return Some(End::Lost { welcomed: true });
                    }
                    self.set_connected(true);
                }
                FromCore::Error(why) => {
                    let _ = self.events.send(Event::Refused(why));
                }
                // a state that does not read is dropped: the panel keeps its picture
                FromCore::State(st) if *welcomed => {
                    if let Ok(st) = State::read(&st) {
                        let _ = self.events.send(Event::State(Box::new(st)));
                    }
                }
                FromCore::Shutdown if *welcomed => {
                    let _ = self.events.send(Event::Shutdown);
                    return Some(End::Stopped);
                }
                _ => {}
            }
        }
        None
    }

    /// A command from the panel; `Some` when the client stops.
    async fn on_command(
        &mut self,
        cmd: Option<Command>,
        write: &mut OwnedWriteHalf,
    ) -> Option<End> {
        match cmd {
            // the last handle is gone, or asked to stop: say bye
            None => {
                let _ = put(write, &ToCore::Bye).await;
                Some(End::Stopped)
            }
            Some(Command::Stop(done)) => {
                let _ = put(write, &ToCore::Bye).await;
                let _ = done.send(());
                Some(End::Stopped)
            }
            Some(Command::Send(msg, reply)) => {
                let sent = self.connected && put(write, &msg).await.is_ok();
                let _ = reply.send(sent);
                None
            }
            Some(Command::Seen(ids)) => {
                if !self.connected || put(write, &ToCore::Seen(ids.clone())).await.is_err() {
                    self.queue_seen(ids);
                }
                None
            }
        }
    }
}

/// Writes one message.
async fn put(write: &mut OwnedWriteHalf, msg: &ToCore) -> std::io::Result<()> {
    let line = encode(&msg.to_json()).map_err(std::io::Error::other)?;
    write.write_all(&line).await
}
