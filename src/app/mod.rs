//! The `panorama tui` app (Python `SwarmApp`): the graph popup and the home tab's view.
//!
//! - [`model`]: its state as the collector's refreshes change it, with no IO: the selection
//!   kept by identity, the arming, the last error, which view it shows;
//! - [`paint`]: a frame of it;
//! - [`actor`]: the event loop; [`collect`]: the collects, on a thread of their own;
//!   [`panic`]: the panic hook (to the log, never onto the view); [`run`]: `panorama tui`.

pub mod actor;
pub mod collect;
pub mod model;
pub mod paint;
pub mod panic;
pub mod run;
