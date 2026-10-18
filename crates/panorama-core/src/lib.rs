//! Pure core of herdr-panorama (plan `docs/plans/rust-rewrite.md` §2.2, §2.3).
//!
//! This crate is the purity boundary: it takes data and the current time and
//! returns data. It does no IO, starts no threads and reads no environment.
//!
//! - [`model`]: the swarm (orchestrators, threads, agents), the status vocabulary and the
//!   agent tree, as the core publishes them to the panels.
//!
//! `view` (rendering into a cell grid) arrives with the per-tab panel (P3).

pub mod model;
