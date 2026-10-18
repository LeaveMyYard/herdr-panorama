//! The swarm model: orchestrators, the threads they drive, and the agents in those threads
//! (Python `model.py`, plan `docs/plans/rust-rewrite.md` §2.3).
//!
//! The types are what the core publishes to the panels in `state.swarm` (protocol
//! `docs/protocol/panel-v1.md`), field for field, so they deserialize from it directly. They
//! are harness-neutral: nothing here knows how a session is read.
//!
//! - [`status`]: the status vocabulary (agent statuses and the derived view states), with
//!   their glyphs, labels and urgency order.
//! - [`swarm`]: the tree itself, and the counts the views show.
//! - [`tree`]: an agent tree inside one thread (who spawned whom).

pub mod status;
pub mod swarm;
pub mod tree;

pub use status::{CLOCK, SPIN, Status};
pub use swarm::{
    Agent, BgTask, Counts, HerdrStatus, Orchestrator, Phase, PhaseState, Swarm, Thread,
    ThreadSource, ThreadStatus,
};
pub use tree::TreeKids;
