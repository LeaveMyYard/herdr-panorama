//! What the interactive views share (plan §2.2's `tui_kit`, kept in the binary until the
//! inbox popup needs it): the per-tab panel and the `panorama tui` app read the same swarm
//! through the same selection rules.
//!
//! - [`selection`]: what is selected, how a refresh keeps it by identity, what a key would
//!   act on, and how long the keys wait after it changed.
//! - [`layout`]: whether the view is the docked panel at its width.
//! - [`term`]: the terminal; [`clock`]: the wall clock and the animation time; [`pacing`]:
//!   when a colour tick draws; [`guard`]: tasks whose panic is caught and reported;
//!   [`crash`]: the crash marker and the panic hook.

pub mod clock;
pub mod crash;
pub mod guard;
pub mod layout;
pub mod pacing;
pub mod selection;
pub mod term;
