//! The `panorama tui` app (Python `SwarmApp`): the graph popup and the home tab's view.
//!
//! - [`model`]: its state as the collector's refreshes change it, with no IO: the selection
//!   kept by identity, the arming, the last error, which view it shows. The actor that feeds
//!   and paints it joins in P4 PR 8a-2.

pub mod model;
