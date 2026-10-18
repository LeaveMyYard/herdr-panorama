//! The client task: connections one after another ([`super::connection`]), with the backoff
//! between them, answering the panel's commands while there is no core.

use super::backoff::Backoff;
use super::{Command, Config, Event};
use tokio::sync::mpsc;

/// How a connection ended.
pub(super) enum End {
    /// Asked to stop, or the core shut the panels down: the task ends.
    Stopped,
    /// Lost (or never made); whether it got as far as the welcome.
    Lost { welcomed: bool },
}

pub(super) struct Task {
    pub(super) cfg: Config,
    pub(super) rx: mpsc::UnboundedReceiver<Command>,
    pub(super) events: mpsc::UnboundedSender<Event>,
    pub(super) pending_seen: Vec<String>,
    pub(super) connected: bool,
}

pub(super) async fn run(
    cfg: Config,
    rx: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<Event>,
) {
    let mut backoff = Backoff::new(&cfg.backoff);
    let mut t = Task {
        cfg,
        rx,
        events,
        pending_seen: vec![],
        connected: false,
    };
    loop {
        let welcomed = match t.session().await {
            End::Stopped => return,
            End::Lost { welcomed } => welcomed,
        };
        t.set_connected(false);
        if t.idle(backoff.next(welcomed)).await {
            return;
        }
    }
}

impl Task {
    pub(super) fn set_connected(&mut self, on: bool) {
        if on != self.connected {
            self.connected = on;
            let _ = self.events.send(Event::Connected(on));
        }
    }

    pub(super) fn queue_seen(&mut self, ids: Vec<String>) {
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
}
