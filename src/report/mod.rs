//! What is left of the explicit thread channel's write side. `report_thread` and
//! `panorama emit` write nothing since threads were removed (plan `uniform-agent-tree.md` §6);
//! only the item cap the Codex channel reader ([`crate::collect::codex`]) still applies is
//! kept, until the readers go too.

/// Phases and agents per thread (and per report list).
pub const MAX_ITEMS: usize = 30;
