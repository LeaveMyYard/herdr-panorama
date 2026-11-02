//! Harness identity in the existing orchestrator title slot. The raw tab label and pane
//! identity remain unchanged for navigation; only presentation adds the established name.

use super::new_agent::HARNESSES;
use crate::model::Orchestrator;

pub fn title(o: &Orchestrator) -> String {
    if super::capture::legacy() { return o.tab_label.clone(); }
    let name = HARNESSES
        .iter()
        .find(|h| h.id == o.harness)
        .map_or(o.harness.as_str(), |h| h.name);
    format!("{name} · {}", o.tab_label)
}
