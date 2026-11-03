//! Bounded, read-only Codex history observation. All turn states describe persisted
//! history, never the original CLI's current runtime state. The collector must retain
//! herdr's pane status. Session-tree IDs are metadata, never lookup keys.
mod cache;
mod discovery;
mod history;
mod hydrate;
pub mod identity;
mod parse;
mod protocol;
mod types;

pub use cache::Observer;
pub use types::{Observation, Options, Thread, Tool, Turn, TurnStatus};

#[cfg(test)]
mod tests;
