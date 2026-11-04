//! The `panorama` binary's modules (plan §2.2): `proto`, the core <-> panel protocol;
//! `settings`, what the panel reads from panorama's and herdr's `config.toml`; `panel`, the
//! per-tab panel viewer; `contract`, what `scripts/panel` and `scripts/tui` check before running it;
//! `machines`, agents on herdr's saved SSH machines; `collect`, the swarm model from herdr and the
//! transcripts; `snapshot`, the `panorama snapshot` command; `ui`, what the panel and the app
//! share; `app`, the `panorama tui` app's model; `core`, the panels' core (server side);
//! `panels`, the per-tab panels' manager; `home`, the home tab; `new_agent`, + New agent.

pub mod app;
pub mod app_feed;
pub mod app_inbox;
pub mod codex;
pub mod collect;
pub mod contract;
pub mod core;
pub mod emit;
pub mod home;
pub mod hook;
pub mod host;
pub mod machines;
pub mod mcp;
pub mod new_agent;
pub mod panel;
pub mod panels;
pub mod produce;
pub mod proto;
pub mod report;
pub mod settings;
pub mod snapshot;
pub mod ui;
pub mod unseen;

/// Python-captured 1.x swarms read as 2.x, for the unit tests (`tests/v1_wire`).
#[cfg(test)]
#[path = "../tests/v1_wire/mod.rs"]
pub(crate) mod v1_wire;
