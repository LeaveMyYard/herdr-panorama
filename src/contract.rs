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
///
/// It is answered as `claude <n>`, never a bare number: a binary from before this contract
/// answers every `contract …` with the panel's bare `1`, and must never pass this probe.
pub const CLAUDE: u32 = 1;

/// The view's contract (`panorama contract tui`): what `scripts/tui` checks before it runs this
/// binary as the manifest's `panorama` and `new-agent` panes, its `open` actions and its startup
/// hook, instead of the Python app. It covers the `tui`, `new-agent`, `open [--popup|--split]`
/// and `home [--startup]` subcommands (2: `open` and `home`), the home view's record and lock (the paths
/// Python's `home` reads), the core it serves and the protocol (1.x, what Python's `open` and
/// `home --startup` speak to it), the + New agent popup's started marker and
/// `PANORAMA_NEW_AGENT_CWD`. Raise it with `TUI_CONTRACT=` in `scripts/tui` when any of these
/// changes incompatibly. Answered as `tui <n>`, like `claude <n>`.
pub const TUI: u32 = 2;

/// The answer to `panorama contract [name]`: the panel's bare number without a name,
/// `claude <n>` for `claude`, `tui <n>` for `tui`, nothing for a name this binary does not know.
pub fn answer(name: Option<&str>) -> Option<String> {
    match name {
        None => Some(PANEL.to_string()),
        Some("claude") => Some(format!("claude {CLAUDE}")),
        Some("tui") => Some(format!("tui {TUI}")),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests;
