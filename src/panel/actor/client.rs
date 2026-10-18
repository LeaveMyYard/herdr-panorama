//! The core client's events: a state, a connection made or lost, shutdown.

use super::{Actor, End};
use crate::panel::input::Painted;
use crate::proto::client::Event;
use std::io;
use std::time::Instant;

impl Actor {
    /// The core client's event; `Some` when the panel ends.
    pub(super) async fn on_client(&mut self, ev: Option<Event>) -> io::Result<Option<End>> {
        let cause = match ev {
            Some(Event::State(st)) => {
                // a switch waits for this echo of its announcement
                self.focus.send_replace(st.focused_pane.clone());
                self.panel.on_state(*st, Instant::now());
                Painted::Refresh
            }
            Some(Event::Connected(on)) => {
                self.panel.on_connected(on);
                self.no_core.connected(on, Instant::now());
                Painted::Other
            }
            Some(Event::Shutdown) => return Ok(Some(End::Shutdown)),
            Some(Event::Refused(_)) => return Ok(None),
            None => return Ok(Some(End::ClientGone)),
        };
        self.repaint(cause).map(|()| None)
    }
}
