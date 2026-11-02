//! Exact Codex thread identifiers. Shared session-tree IDs are never a substitute.

/// The canonical, lower-case UUID spelling emitted by Codex's ThreadId.
pub fn valid_thread_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}

mod bind;
mod cli;
mod owner;
mod root;
pub use cli::{hook, manual};

#[cfg(test)]
mod tests;

mod store;
pub use store::resolve;
