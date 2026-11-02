# Codex in the Rust Panorama view

The Rust UI and core show local Claude Code and Codex panes together. Each pane
keeps its harness label, tab, focus target, and herdr status. Arrow navigation,
clicks, and Enter use the existing pane IDs. Missing Codex history or registration
does not remove the pane.

This feature is in the **Rust binary**. During the port, `uv run panorama` runs the
Python implementation and remains Claude-only. Build the Rust binary according
to [CONTRIBUTING.md](../CONTRIBUTING.md), and use its path for the commands below,
for example `target/release/panorama snapshot --json`. Building in an isolated
checkout does not install, restart, or activate a running plugin.

## Launching and associating a session

Choose **Codex** in the Rust **New agent** form. Panorama starts it through
herdr's agent launcher with `--no-daemon`, in its own tab and, when configured,
beside its panel. It retains the same readiness checks and safe failed-start
cleanup as Claude Code.

An exact thread identity is required for history. Panorama uses a native
`herdr:codex` session ID when herdr supplies one. Otherwise, it can use a
Panorama-local association that names the exact root thread and records the
foreground Codex process's PID and start identity. It checks current foreground
ownership again before consuming that association. Working directory, newest
rollout, and the Codex session-tree group ID are never identity fallbacks.

Panorama's association commands **do not write herdr session or lifecycle
state** and do not configure native herdr resume. Their records live under
`$XDG_STATE_HOME/herdr-panorama/codex-bindings` (normally
`~/.local/state/herdr-panorama/codex-bindings`). Invalid, conflicted, untrusted,
or stale records leave the pane visible without details.

### Optional automatic registration

Merge the entry from [codex/hooks.json](../codex/hooks.json) into the appropriate
Codex hook configuration, replacing `panorama` in the command with the absolute
path of the Rust binary. The entry handles root `SessionStart` events for startup,
resume, and fork. Review and trust the exact hook definition with Codex's `/hooks`
interface. Panorama never installs it or edits your user configuration for you.

Codex 0.166.0 enables hooks by default. User hooks can live in
`~/.codex/hooks.json`, or beside `config.toml` in a custom `CODEX_HOME`; trusted
projects can also provide `.codex/hooks.json`. See
[Codex's hook configuration documentation](https://learn.chatgpt.com/docs/hooks#where-codex-looks-for-hooks)
for configuration layers and trust requirements.

Automatic association requires an embedded foreground Codex process, started as
`codex --no-daemon`, and fresh proof that the hook belongs to that exact pane.
The hook does not trust inherited `HERDR_PANE_ID` alone: the default shared
Codex daemon can carry another pane's environment. Child hooks cannot replace
the root association. Hook output remains a benign `{}` and supplies no model
instructions or transcript content.

The hook accepts an unset association or the same root again. A different root
invalidates the old local association and requires explicit rebinding; an old or
delayed hook cannot silently select a different thread. Startup metadata may not
have been persisted yet, in which case the hook does nothing and registration
can be retried explicitly after the thread is readable.

### Existing sessions and explicit binding

For an existing shared-daemon session, select both the exact root thread ID and
the intended pane explicitly:

```sh
/absolute/path/to/panorama codex-bind THREAD_ID --pane PANE_ID
```

The command verifies root metadata and the chosen pane's current foreground
Codex process before persisting the association. Omitting `--pane` is accepted
only when the command itself can prove embedded Codex ownership.

**Rebind after switching roots in the same TUI process**, including an in-process
new conversation, fork, or session switch. An explicit shared-daemon mapping is
an operator-selected association, not native detection of which thread that TUI
is currently displaying. Its PID/start check detects process replacement, not a
conversation switch inside the same process. A conflicting native herdr ID is
never overridden by the local store.

## What the details mean

One Codex session card contains its recorded turns as chronological phases and
its exact parent/child agent tree. It shows bounded prompt/summary text, model,
the latest recorded tool, and the tool count in the loaded turn window. Counts
are not all-time totals. Explicit `report_thread` declarations can organize the
same children without duplicating them or replacing the root card.

Herdr supplies the root pane's runtime state. Persisted completed, failed, or
interrupted turns describe historical work. A completed turn, idle pane, or
observer `notLoaded` status does **not** establish that the session completed.
Child runtime state remains unknown when only persisted history is available;
failed/interrupted child turns are identified as such in their last message.

The read-only adapter honors `CODEX_HOME` and supports legacy JSONL and paginated
history through the app-server's read APIs. It probes capabilities and falls
back to full turn reads where needed: Codex 0.166.0 supports `thread/items/list`
for paginated history but not legacy history. A generated schema alone is not
treated as proof that a method works for every storage mode.

Interactive collection never waits for the history worker. Details can appear
on a subsequent refresh. The standalone Rust `snapshot` command gives all panes
one shared 750ms history warmup budget, then returns what is available and stops
its owned app-server. Its ownership lookups have a separate shared 100ms budget
per collection, with at most 50ms per request and no foreground cache reused
across collections. Slow or unavailable sources produce a visible bare pane.

Reads are bounded and partial history can omit older turns or children. Only a
fresh, complete, verified descendant graph permits reading child report files.
Each child's reports can claim only its own verified subtree, root reports take
precedence, and declaration IDs include their source thread to prevent sibling
collisions. Report reads are capped per collection and do not read arbitrary
paths supplied in a report.

## Optional `report_thread` MCP server

Merge [codex/config.example.toml](../codex/config.example.toml) into your chosen
Codex configuration only when opting in. Set `command` to the absolute Rust
binary path:

```toml
[mcp_servers.herdr_panorama]
command = "/absolute/path/to/panorama"
args = ["mcp"]
```

Codex supplies the exact caller in each MCP call's `_meta.threadId`. Panorama
uses it separately for root and child callers; the shared `_meta.sessionId` is
not an identity key. An invalid supplied thread ID is refused. MCP reporting
does not register or change the pane association.

From a Codex shell tool, the Rust `emit` command can resolve the exact
`CODEX_THREAD_ID`, or you can pass `--session` explicitly:

```sh
/absolute/path/to/panorama emit --session THREAD_ID --thread work --status active
```

`CODEX_SESSION_ID` is deliberately ignored because it may identify the shared
session tree. Existing Claude reporting keeps its own fallback behavior.

## Compatibility and limits

Rust model and wire messages add a defaulted harness field: old messages read as
Claude, and explicit harness values survive Rust round trips. Older Python
readers can ignore the extra field but do not preserve it on re-emission.

Remote Codex panes remain visible from herdr metadata; detailed remote history
is a follow-up, and a remote pane never reads this machine's local Codex home or
association store. Inbox answer delivery to Codex is also a follow-up. Optional
MCP reporting, pane visibility, and session registration do not enable it.
