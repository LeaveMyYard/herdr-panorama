//! The home tab's label (Python `home.is_home_label`, `neutral_label`).

use super::HOME_LABEL;
use crate::panels::snap::{Snap, SnapPane};

/// `◆ panorama`, also behind herdr-inbox's `❓N ` prefixes (`^(?:❓\d+\s+)+`).
pub fn is_home_label(label: &str) -> bool {
    let mut rest = label;
    while let Some(after) = rest.strip_prefix('❓') {
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let spaced = after[digits..].trim_start();
        if digits == 0 || spaced.len() == after[digits..].len() {
            break; // not a whole `❓N ` mark: the label as it is
        }
        rest = spaced;
    }
    rest == HOME_LABEL
}

/// The label a tab gets when panorama is no longer in it: the remaining agent's herdr name,
/// else its kind (`claude`), else `tab`.
pub fn neutral_label(snap: &Snap, panes: &[&SnapPane]) -> String {
    let name_of = |p: &SnapPane| {
        snap.agent_names
            .iter()
            .find(|(id, _)| *id == p.pane_id)
            .map(|(_, n)| n.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| p.name.clone().filter(|n| !n.is_empty()))
    };
    let agents: Vec<&&SnapPane> = panes
        .iter()
        .filter(|p| p.agent.as_deref().is_some_and(|a| !a.is_empty()))
        .collect();
    agents
        .iter()
        .find_map(|p| name_of(p))
        .or_else(|| agents.first().and_then(|p| p.agent.clone()))
        .unwrap_or_else(|| "tab".into())
}
