//! The switch's announcement to the core (`act::switch::Announce`): `focusing` through the
//! client, and its echo in the states the actor publishes on a watch.

use crate::panel::act::switch::Announce;
use crate::proto::client::Client;
use std::time::Duration;
use tokio::sync::{Mutex, watch};

/// The client and the focused pane of the latest state.
pub struct Announcer {
    client: Client,
    focus: Mutex<watch::Receiver<Option<String>>>,
}

impl Announcer {
    pub fn new(client: Client, focus: watch::Receiver<Option<String>>) -> Self {
        Self {
            client,
            focus: Mutex::new(focus),
        }
    }
}

impl Announce for Announcer {
    async fn focusing(&self, pane: &str) -> bool {
        // what the states said so far is old: only an echo from here on counts
        self.focus.lock().await.borrow_and_update();
        self.client.focusing(pane).await
    }

    async fn echoed(&self, pane: &str, within: Duration) -> bool {
        let mut focus = self.focus.lock().await;
        let deadline = tokio::time::Instant::now() + within;
        loop {
            // an echo that came since the announcement counts (`has_changed` marks it)
            if focus.has_changed().unwrap_or(false)
                && focus.borrow_and_update().as_deref() == Some(pane)
            {
                return true;
            }
            match tokio::time::timeout_at(deadline, focus.changed()).await {
                Ok(Ok(())) => {
                    if focus.borrow_and_update().as_deref() == Some(pane) {
                        return true;
                    }
                }
                _ => return false,
            }
        }
    }
}

#[cfg(test)]
mod tests;
