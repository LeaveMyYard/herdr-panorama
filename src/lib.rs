//! The `panorama` binary's modules (plan §2.2): `proto`, the core <-> panel protocol;
//! `settings`, what the panel reads from panorama's and herdr's `config.toml`; `panel`, the
//! per-tab panel viewer; `contract`, the number `scripts/panel` checks before running it;
//! `machines`, agents on herdr's saved SSH machines; `ui`, what the panel and the app share;
//! `app`, the `panorama tui` app's model.

pub mod app;
pub mod contract;
pub mod machines;
pub mod panel;
pub mod proto;
pub mod settings;
pub mod ui;
pub mod unseen;
