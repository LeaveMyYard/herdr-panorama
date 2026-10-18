//! Whether anything on screen moves (Python `render.animated`): spinners, waiting clocks,
//! blinking needs-you states and the unseen pulse. When nothing does, animation ticks need
//! no new frame.

use super::unseen::Marks;
use crate::model::{HerdrStatus, Status, Swarm};

/// Something in `swarm` (or one of the unseen marks) animates.
pub fn animated(swarm: &Swarm, unseen: &Marks) -> bool {
    let all = || swarm.orchestrators.iter().flat_map(|o| o.agents());
    if !unseen.is_empty() && all().any(|a| unseen.contains_key(&a.id)) {
        return true;
    }
    swarm.orchestrators.iter().any(|o| {
        matches!(o.herdr_status, HerdrStatus::Working | HerdrStatus::Blocked)
            || !o.inbox.is_empty()
            || !o.bg.is_empty()
            || o.agents().any(|a| {
                matches!(
                    a.status,
                    Status::Running
                        | Status::Failed
                        | Status::Blocked
                        | Status::Waiting
                        | Status::Background
                )
            })
    })
}

#[cfg(test)]
mod tests;
