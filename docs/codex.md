# Codex in the Rust Panorama view

The Rust UI and core show local Claude Code and Codex panes together. Each pane
keeps its harness identity, tab, focus target, and herdr status. Arrow navigation,
clicks, and Enter use the existing pane IDs. Missing Codex history or registration
does not remove the pane.

This feature is in the **Rust binary**. During the port, `uv run panorama` runs the
Python implementation and remains Claude-only. Build the Rust binary according
to [CONTRIBUTING.md](../CONTRIBUTING.md), and use its path for the commands below,
for example `target/release/panorama snapshot --json`. Building in an isolated
checkout does not install, restart, or activate a running plugin.

## Harness icons

Root titles can use the real Claude and Codex marks from **Herdr Agent
Icons Max** (U+E1A0 and U+E1A1), followed by the title. Launcher choices and
help keep their full harness names. The font is distributed by
[herdr-radar](https://github.com/hhdebb/herdr-radar) and
[herdr-icon-agent-ui](https://github.com/qintmb/herdr-icon-agent-ui); Panorama
does not bundle fonts or change terminal configuration.

In Panorama's plugin `config.toml`, `harness_icons = "auto"` is the default.
Auto conservatively recognizes local Ghostty or its herdr wrapper
(`TERM_PROGRAM=ghostty` or `herdr`, without
`SSH_CONNECTION`) with the standard `font-codepoint-map =
U+E1A0-U+E1BA=Herdr Agent Icons Max` in its XDG `ghostty/config` or
`ghostty/config.ghostty` and a nonempty `HerdrAgentIconsMax*.ttf` in
`~/.local/share/fonts`, `~/.fonts`, or `~/Library/Fonts`. It samples bounded
local files once at startup; restart the view after changing font setup. Config
includes, custom font locations and other terminals fall back to text.

Installed fonts and config files are hints, not proof of the active terminal's
mapping. After installing the font and configuring your terminal to map these
codepoints, use `harness_icons = "font"` to opt in explicitly. If glyphs appear
as boxes or the mapping is unavailable, `harness_icons = "text"` restores
`Claude Code ·` / `Codex ·`. These explicit settings are re-read with the other
view settings. Unknown harnesses always retain their textual identifier.

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

The hook entry must actually be configured and trusted to register new sessions;
`--no-daemon` alone does not install it. Existing shared-daemon panes remain bare
until explicitly associated with their exact root and pane. Other panes never
inherit a neighboring pane's association.

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

A Codex root is its pane: its own state (herdr's status, inbox questions) is on
the root, and no agent stands for the root thread. Below it is its exact
parent/child agent tree, at any depth; the root's own children have no parent.
Each agent shows bounded prompt/summary text, model, the latest recorded tool,
and the tool count in the loaded turn window. Counts are not all-time totals.
Turns are not phases (plan `uniform-agent-tree.md` U3). The compact dock shows one tile per
active direct subagent; the docked entry lists two layers of agents, with deeper agents as
status glyphs on the right of their row. Open the expanded graph to navigate full
agent cards by depth; the narrow graph offers selectable agent pods and the
selected agent's details.
History-only children stay unknown, including in the compact dock.

### How Codex agents are named

A Codex root is named by, in order: a pane name you set in herdr; its tab's
label, when you named the tab and the tab holds only this agent; the root
thread's `name` (from `/rename` or Codex's own title); the first line of its
first prompt (Codex's thread `preview`, else the earliest prompt read); a
telling terminal title; else
`Codex · tab N`.

A subagent, at any depth, is named by, in order:

1. **The task name its spawner gave it.** Codex 0.166.0's app-server schema
   (`codex app-server generate-json-schema`) puts it in the spawned thread's own
   `thread/read` record, as the last segment of
   `source.subAgent.thread_spawn.agent_path` (`/root/review_auth` reads as
   `review auth`). Codex sets the path only when the spawner passes a
   `task_name`; older spawns have none.
2. Its thread's `name`, when set.
3. Codex's `agentNickname` (`Euclid`, `Lovelace`, …). It is stable but it is
   Codex's choice, not the spawner's.
4. The first line of its first prompt (the thread's `preview`, else the
   earliest prompt read).
5. Its `agentRole` and short thread id, else `agent <short id>`.

The spawn item itself (`collabAgentToolCall`) carries the spawner's whole
`prompt`, not a name; Panorama reads only its receiver ids, never its prompt.
Naming reads nothing beyond the thread records Panorama already fetches, so it
does not change any read budget.

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
per collection spent only inside ownership lookups, with at most 50ms per
request and no foreground cache reused
across collections. Slow or unavailable sources produce a visible bare pane.

Display reads keep the newest 128 items and at most 16 turns. Child ancestry has
a separate progressive scan: each refresh can continue up to two additional
64-item pages beyond that display window. Supported full-turn fallback reads
one oldest-first turn page, up to 256 ancestry items per refresh; legacy full
reads scan the same bounded oldest-first slice. Large sessions can therefore
need several refreshes before older children appear. Progress and candidate IDs
are cached by exact root, and every displayed child is freshly read and checked
against its exact parent on each refresh. Aging out of the recent item window
does not by itself remove a known child or mark it done.

The observer still reads at most eight threads per refresh (root included), with
bounded per-root scan caches, 64 candidate IDs per scanned thread, continuation
limits, the existing whole-refresh deadline and wire byte limits. Known valid
candidates get lookup priority while unverified siblings rotate. A bounded target
path also schedules work across depths. Attempts and parent hints are scoped to
the expected parent-child edge, so a wrong-scope reference cannot age or erase
the valid edge; each ancestor must freshly verify before
its descendant can appear. Rejected metadata never allocates persisted scan state,
so foreign references cannot reset a valid branch's older backfill. Oversized graphs,
truncated display history, unsupported/failed reads and exhausted limits remain
partial. Cached ancestry never refreshes metadata proof merely by reuse. Only a
Report files are no longer read at all.

## Removed: the `report_thread` MCP server

Threads were removed, so there is nothing to report: Panorama shows every agent
and subagent as one tree, under the name its spawner gave it. The example
`[mcp_servers.herdr_panorama]` block is gone from
[codex/config.example.toml](../codex/config.example.toml). If you merged it into
your Codex configuration, delete it there yourself; Panorama never edits Codex
configuration. Until you do, `panorama mcp` keeps answering for the rest of this
release: `report_thread` is still listed, records nothing and answers
`{"status": "ignored", "reason": "threads were removed"}`, whatever caller
`_meta.threadId` it carries. The Rust `panorama emit` likewise records nothing and
says so on stderr. A later release removes both.

Report files an earlier version wrote under the state directory's
`herdr-panorama/events/` are left in place; the Rust binary no longer writes
or reads them (only the frozen Python entrypoint still does), and you can
delete them.

## Compatibility and limits

Rust model and wire messages add a defaulted harness field: old messages read as
Claude, and explicit harness values survive Rust round trips. Older Python
readers can ignore the extra field but do not preserve it on re-emission.

Remote Codex panes remain visible from herdr metadata; detailed remote history
is a follow-up, and a remote pane never reads this machine's local Codex home or
association store. Inbox answer delivery to Codex is also a follow-up. Pane visibility and session
registration do not enable it.
