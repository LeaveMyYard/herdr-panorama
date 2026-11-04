//! The core client's events: a state, a connection made or lost, shutdown, a refused hello.

use super::{Actor, End};
use crate::panel::input::Painted;
use crate::proto::client::Event;
use std::io;
use std::path::Path;
use std::time::Instant;

/// The core refused this panel's hello (`why`, as the core said it): a core of another protocol
/// major is still running, typically after an upgrade (`docs/protocol/panel-v2.md`,
/// "Version 1"). The panel keeps waiting for a core it can speak to, as before; this says why
/// in `<state dir>/panel-launch.log`, next to the launcher's own fallback note.
pub fn note_refused(state: &Path, why: &str, stamp: &str) {
    let msg = format!(
        "{stamp} panorama: the core refused this panel ({why}; it speaks protocol {}): \
         restart the panorama home view after an upgrade\n",
        crate::proto::PROTOCOL
    );
    let _ = std::fs::create_dir_all(state);
    let _ = std::fs::write(state.join("panel-launch.log"), msg);
}

impl Actor {
    /// The core client's event; `Some` when the panel ends.
    pub(super) async fn on_client(&mut self, ev: Option<Event>) -> io::Result<Option<End>> {
        let cause = match ev {
            Some(Event::State(st)) => {
                // a switch waits for this echo of its announcement
                self.focus.send_replace(st.focused_pane.clone());
                self.driver.state(*st, Instant::now())
            }
            Some(Event::Connected(on)) => {
                self.driver.panel.on_connected(on);
                self.no_core.connected(on, Instant::now());
                Painted::Other
            }
            Some(Event::Shutdown) => return Ok(Some(End::Shutdown)),
            Some(Event::Refused(why)) => {
                if !std::mem::replace(&mut self.refused, true) {
                    note_refused(&self.place.state, &why, &crate::ui::clock::stamp());
                }
                return Ok(None);
            }
            None => return Ok(Some(End::ClientGone)),
        };
        self.repaint(cause).map(|()| None)
    }
}

#[cfg(test)]
mod tests;
