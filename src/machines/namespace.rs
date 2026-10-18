//! A machine's swarm in the local view (Python `machines.namespace`, `prefix`, `local_id`):
//! herdr ids are per server, so every id of another machine carries that machine's id.

use super::Machine;
use panorama_core::model::Swarm;

/// The namespace of a machine's herdr ids: its stable id (labels are for display and can
/// repeat or contain `:`).
pub fn prefix(machine: &Machine) -> String {
    format!("{}:", machine.id)
}

/// A namespaced pane id back to the id on its own server.
pub fn local_id(pane_id: &str, machine_id: &str) -> String {
    pane_id
        .strip_prefix(&format!("{machine_id}:"))
        .unwrap_or(pane_id)
        .to_owned()
}

/// Every orchestrator gets its machine (`machine_id` for identity and commands, `machine` the
/// label for display) and `<machine id>:` ids. The inbox is local, so remote orchestrators
/// carry no inbox questions; panorama never docks into a remote server.
pub fn namespace(mut swarm: Swarm, machine: &Machine) -> Swarm {
    let p = prefix(machine);
    for o in &mut swarm.orchestrators {
        o.machine = Some(machine.label.clone());
        o.machine_id = Some(machine.id.clone());
        o.pane_id = format!("{p}{}", o.pane_id);
        if let Some(tab) = o.tab_id.as_mut().filter(|t| !t.is_empty()) {
            *tab = format!("{p}{tab}");
        }
        o.inbox.clear();
        o.inbox_texts.clear();
    }
    swarm.docked = None;
    swarm
}

#[cfg(test)]
mod tests;
