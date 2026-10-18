//! An orchestrator on another herdr machine (Python `SwarmApp._goto_remote`). herdr's API
//! cannot switch this herdr to another machine, so the pane is focused on that machine's own
//! server (in front once herdr shows that machine) and the operator is told where it is.

use herdr_agent_kit::herdr_api::{Herdr, PaneId, clean};

/// The notice's title.
pub const TITLE: &str = "another machine";

/// A namespaced pane id back to the id on its own server (Python `machines.local_id`).
pub fn local_id(pane: &str, machine_id: &str) -> String {
    pane.strip_prefix(&format!("{machine_id}:"))
        .unwrap_or(pane)
        .to_owned()
}

/// Focuses `pane` on machine `machine_id` (labelled `machine`); the notice's text.
pub async fn goto_remote(
    h: &Herdr,
    machine_id: &str,
    machine: &str,
    pane: &str,
    label: &str,
) -> String {
    let local = PaneId::new(local_id(pane, machine_id));
    let text = match h.machine_focus_agent(machine_id, &local).await {
        Ok(()) => format!("{label} is on {machine}: switch herdr to {machine} to see it"),
        Err(e) => format!("could not focus it on {machine}: {e}"),
    };
    clean(&text, 300)
}

#[cfg(test)]
mod tests;
