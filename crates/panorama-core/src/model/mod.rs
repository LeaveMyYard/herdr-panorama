//! The swarm model: roots (agent sessions in herdr panes) and every agent they spawned, as one
//! tree (plan `docs/plans/uniform-agent-tree.md` §2.1).
//!
//! The types are what the core publishes to the panels in `state.swarm` (protocol
//! `docs/protocol/panel-v2.md`), field for field, so they deserialize from it directly. They
//! are harness-neutral: nothing here knows how a session is read.
//!
//! - [`status`]: the status vocabulary (agent statuses and the derived view states), with
//!   their glyphs, labels and urgency order.
//! - [`swarm`]: the tree itself, and the counts the views show.
//! - [`tree`]: a root's agent tree (who spawned whom).

pub mod status;
pub mod swarm;
pub mod tree;

pub use status::{CLOCK, SPIN, Status};
pub use swarm::{Agent, BgTask, Counts, HerdrStatus, Root, Swarm};
pub use tree::TreeKids;
