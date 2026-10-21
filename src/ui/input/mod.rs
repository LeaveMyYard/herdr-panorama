//! The input both views share (Python `SwarmApp`'s keys, mouse and arming, which `PanelApp`
//! inherits): the key bindings, the [`Effect`]s an input asks the actor for, and when keys and
//! clicks act. No IO: every method takes the time.
//!
//! - [`host`]: what a view supplies (Python: what `PanelApp` overrides);
//! - [`actions`]: what the action keys do; [`clicks`]: what a click does;
//! - [`keys`]: the bindings; [`effect`]: what the actor carries out; [`arming`]: when a click
//!   acts.

pub mod actions;
pub mod arming;
pub mod clicks;
pub mod effect;
pub mod host;
pub mod keys;

pub use actions::Actions;
pub use arming::{ClickArming, Painted};
pub use clicks::Clicks;
pub use effect::{Effect, Toast};
pub use host::Host;
pub use keys::{Key, key};

#[cfg(test)]
mod tests;
