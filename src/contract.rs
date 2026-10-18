//! The panel contract (`panorama contract`): the number `scripts/panel` checks before it runs
//! this binary as a per-tab panel, so a stale or broken build falls back to the Python viewer
//! instead of dying at start (docs/plans/rust-rewrite.md §5.4).
//!
//! It covers what the Python core next to the binary relies on: the `panel` subcommand, the
//! core protocol's major ([`crate::proto::PROTOCOL`]), the plugin environment it reads
//! (`HERDR_PANE_ID`, `HERDR_TAB_ID`, `HERDR_PLUGIN_STATE_DIR`, …) and the crash marker
//! (`panel-crash-<pane>`). Raise it with `CONTRACT=` in `scripts/panel` when any of these
//! changes incompatibly.

/// The panel contract this binary implements.
pub const PANEL: u32 = 1;

/// The Claude Code side's contract (`panorama contract claude`): what `hooks/run-hook.sh` and
/// `scripts/mcp` check before they run this binary instead of the Python hooks and MCP server.
/// It covers the `hook <kind>` and `mcp` subcommands, the hook payloads read and the output and
/// exit status (always 0), the hook record format (v1), the MCP tool's name, text and schemas,
/// and the events format (v1). Raise it with `CLAUDE_CONTRACT=` in both scripts when any of
/// these changes incompatibly.
pub const CLAUDE: u32 = 1;

#[cfg(test)]
mod tests;
