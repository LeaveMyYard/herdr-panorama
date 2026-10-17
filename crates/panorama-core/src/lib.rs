//! Pure core of herdr-panorama (plan `docs/plans/rust-rewrite.md` §2.2, §2.3).
//!
//! This crate is the purity boundary: it takes data and the current time and
//! returns data. It does no IO, starts no threads and reads no environment.
//! `model` (swarm, threads, status codes) and `view` (rendering into a buffer)
//! arrive in later phases.
