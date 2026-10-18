//! The client task: connections one after another, with the backoff between them.

use super::{Command, Config, Event};
use crate::proto::framing::{LineReader, encode};
use crate::proto::messages::{FromCore, ToCore};
use crate::proto::socket;
use crate::proto::state::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::mpsc;

/// How a connection ended.
enum End {
    /// Asked to stop, or the core shut the panels down: the task ends.
    Stopped,
    /// Lost (or never made); whether it got as far as the welcome.
    Lost { welcomed: bool },
}

struct Task {
    cfg: Config,
    rx: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<Event>,
    pending_seen: Vec<String>,
    connected: bool,
}

pub(super) async fn run(
    cfg: Config,
    rx: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<Event>,
) {
    let mut t = Task {
        cfg,
        rx,
        events,
        pending_seen: vec![],
        connected: false,
    };
    let mut tries = 0usize;
    loop {
        match t.session().await {
            End::Stopped => return,
            End::Lost { welcomed } => {
                if welcomed {
                    tries = 0;
                }
            }
        }
        t.set_connected(false);
        let wait = t.cfg.backoff[tries.min(t.cfg.backoff.len() - 1)];
        if t.idle(wait).await {
            return;
        }
        tries += 1;
    }
}

impl Task {
    fn set_connected(&mut self, on: bool) {
        if on != self.connected {
            self.connected = on;
            let _ = self.events.send(Event::Connected(on));
        }
    }

    fn queue_seen(&mut self, ids: Vec<String>) {
        for id in ids {
            if !self.pending_seen.contains(&id) {
                self.pending_seen.push(id);
            }
        }
    }

    /// Waits `wait` without a core, answering commands meanwhile. True: stop.
    async fn idle(&mut self, wait: std::time::Duration) -> bool {
        let deadline = tokio::time::sleep(wait);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                () = &mut deadline => return false,
                cmd = self.rx.recv() => match cmd {
                    None => return true,
                    Some(Command::Stop(done)) => {
                        let _ = done.send(());
                        return true;
                    }
                    Some(Command::Send(_, reply)) => {
                        let _ = reply.send(false);
                    }
                    Some(Command::Seen(ids)) => self.queue_seen(ids),
                },
            }
        }
    }

    /// One connection, until it ends.
    async fn session(&mut self) -> End {
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
        let mut buf = vec![0u8; 65536];
        loop {
            tokio::select! {
                got = read.read(&mut buf) => {
                    let n = match got {
                        Ok(0) | Err(_) => return End::Lost { welcomed },
                        Ok(n) => n,
                    };
                    let Ok(msgs) = reader.feed(&buf[..n]) else {
                        return End::Lost { welcomed }; // a line over the limit
                    };
                    for msg in msgs {
                        match FromCore::read(msg) {
                            FromCore::Welcome => {
                                welcomed = true;
                                let pending = std::mem::take(&mut self.pending_seen);
                                if !pending.is_empty() && put(&mut write, &ToCore::Seen(pending.clone())).await.is_err() {
                                    self.queue_seen(pending);
                                    return End::Lost { welcomed };
                                }
                                self.set_connected(true);
                            }
                            FromCore::Error(why) => {
                                let _ = self.events.send(Event::Refused(why));
                            }
                            FromCore::State(st) if welcomed => {
                                // a state that does not read is dropped: the panel keeps its picture
                                if let Ok(st) = State::read(&st) {
                                    let _ = self.events.send(Event::State(Box::new(st)));
                                }
                            }
                            FromCore::Shutdown if welcomed => {
                                let _ = self.events.send(Event::Shutdown);
                                return End::Stopped;
                            }
                            _ => {}
                        }
                    }
                }
                cmd = self.rx.recv() => match cmd {
                    None => {
                        let _ = put(&mut write, &ToCore::Bye).await;
                        return End::Stopped;
                    }
                    Some(Command::Stop(done)) => {
                        let _ = put(&mut write, &ToCore::Bye).await;
                        let _ = done.send(());
                        return End::Stopped;
                    }
                    Some(Command::Send(msg, reply)) => {
                        let sent = self.connected && put(&mut write, &msg).await.is_ok();
                        let _ = reply.send(sent);
                    }
                    Some(Command::Seen(ids)) => {
                        if !self.connected || put(&mut write, &ToCore::Seen(ids.clone())).await.is_err() {
                            self.queue_seen(ids);
                        }
                    }
                },
            }
        }
    }
}

/// Writes one message.
async fn put(write: &mut OwnedWriteHalf, msg: &ToCore) -> std::io::Result<()> {
    let line = encode(&msg.to_json()).map_err(std::io::Error::other)?;
    write.write_all(&line).await
}
