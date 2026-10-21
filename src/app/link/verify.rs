//! The core's pane check for the home view (Python `SwarmApp._owned_pane`): a viewer's hello
//! names its pane, trusted only if herdr says that pane's foreground runs `panorama panel` as
//! the connecting process itself (docs/DESIGN.md M2). Any doubt is no (fail closed).

use crate::core::Verify;
use herdr_agent_kit::herdr_api::{Herdr, PaneId, ProcessInfo};
use std::path::Path;

/// Whether `info` shows `panorama <command>` as process `pid` itself (`panel` for a viewer:
/// the home view's `panorama tui` is no viewer).
pub fn runs_as(info: &ProcessInfo, command: &str, pid: i32) -> bool {
    info.foreground_processes.iter().flatten().any(|p| {
        p.pid == Some(i64::from(pid))
            && p.argv.windows(2).any(|w| {
                Path::new(&w[0]).file_name().and_then(|n| n.to_str()) == Some("panorama")
                    && w[1] == command
            })
    })
}

/// herdr's word on a viewer's pane.
pub struct HerdrVerify(pub Herdr);

impl Verify for HerdrVerify {
    async fn verify(&self, pane: &str, pid: i32) -> bool {
        match self.0.process_info(&PaneId::new(pane)).await {
            Ok(info) => runs_as(&info, "panel", pid),
            Err(_) => false, // herdr down or a reply that does not read: unverified
        }
    }
}
