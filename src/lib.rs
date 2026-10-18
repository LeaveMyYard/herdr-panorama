//! The `panorama` binary's modules (plan §2.2): `proto`, the core <-> panel protocol;
//! `settings`, what the panel reads from panorama's and herdr's `config.toml`; `panel`, the
//! per-tab panel viewer; `contract`, the number `scripts/panel` checks before running it;
//! `machines`, agents on herdr's saved SSH machines; `collect`, the swarm model from herdr and the
//! transcripts; `snapshot`, the `panorama snapshot` command; `ui`, what the panel and the app
//! share; `app`, the `panorama tui` app's model; `core`, the panels' core (server side).

pub mod app;
pub mod collect;
pub mod contract;
pub mod core;
pub mod hook;
pub mod machines;
pub mod mcp;
pub mod panel;
pub mod panels;
pub mod proto;
pub mod report;
pub mod settings;
pub mod snapshot;
pub mod ui;
