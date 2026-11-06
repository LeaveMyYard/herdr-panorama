//! Host adapters: exact terminal observations feed the existing collector.
pub mod collect;
pub mod command;
pub mod context;
pub mod core;
pub mod link;
pub mod records;
pub mod source;

pub use context::{Context, Host};
pub use core::Core;
pub use source::{Observation, SnapshotSource};
