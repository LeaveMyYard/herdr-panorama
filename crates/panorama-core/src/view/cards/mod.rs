//! The graph's card pieces (Python `render.py`, shared by the narrow layout now and the wide
//! graph in P4): boxes and the lines that join them, the animated edge, the `✗ ! ?N` marks,
//! a thread's phases, a row of agent pods, an agent's details, and its action hints.

pub mod actions;
pub mod boxes;
pub mod details;
pub mod edge;
pub mod marks;
pub mod phases;
pub mod pods;

/// A card's box on a screen: `w` × `h` cells at `(x, y)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[cfg(test)]
mod tests;
