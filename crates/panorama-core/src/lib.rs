//! Pure core of herdr-panorama (plan `docs/plans/rust-rewrite.md` §2.2, §2.3).
//!
//! This crate is the purity boundary: it takes data and the current time and
//! returns data. It does no IO, starts no threads and reads no environment.
//!
//! - [`model`]: the swarm (orchestrators, threads, agents), the status vocabulary and the
//!   agent tree, as the core publishes them to the panels.
//!
//! - [`view`]: the views rendered into a cell grid with click targets: the docked panel
//!   (design 9e) the per-tab panel shows; the graph views join in P4.

pub mod model;
pub mod view;
