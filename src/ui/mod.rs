//! What the interactive views share (plan §2.2's `tui_kit`, kept in the binary until the
//! inbox popup needs it): the per-tab panel and the `panorama tui` app read the same swarm
//! through the same selection rules.
//!
//! - [`selection`]: what is selected, how a refresh keeps it by identity, what a key would
//!   act on, and how long the keys wait after it changed.
//! - [`layout`]: whether the view is the docked panel at its width.

pub mod layout;
pub mod selection;
