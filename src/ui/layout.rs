//! Which layout a view shows at its width (Python `SwarmApp._update_dock`'s rule, shared by
//! the panel and the app).

use panorama_core::model::Swarm;
use panorama_core::view::empty::WIDE;

/// The docked panel (design 9e) rather than the graph: an orchestrator is docked here and
/// the pane (`width`, 0 before the first paint) is narrower than the wide layout, or a switch
/// is running (`switching`: herdr may resize the pane meanwhile; Python `_update_dock`).
pub fn docked(swarm: &Swarm, width: i32, switching: bool) -> bool {
    swarm.docked.is_some() && width > 0 && (width < WIDE || switching)
}

#[cfg(test)]
mod tests;
