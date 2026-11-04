# Core-host observations and hook records

`HC_TERMINAL_ID` selects herdr-core before `HERDR_PANE_ID` selects the TUI;
empty variables are absent. With neither, only a live, terminal-aware session
record can select core. Otherwise the existing TUI source remains the default.
The source adapters implement `host::SnapshotSource`; the TUI adapter uses the
existing `session.snapshot` read and preserves its model/serializer behavior.

The core adapter reads `hc ls --json`. `tests/fixtures/hc-list.json` preserves
the envelope and fields observed with installed **hc 0.1.17** on 2026-11-03;
addresses, cwd and `where` are synthetic. Status/harness cases are synthetic
coverage values within that recorded shape. The actual CLI envelope is
`{hidden_finished, terminals:[{address,agent,cwd,state,status,turn,where}]}`.
This differs from `hc schema`'s RPC Terminal object: the CLI list's status is a
plain word or null. The adapter retains live Claude Code/Codex roots only;
unsupported programs stay in the app's stock agent list. `where` is display
context, never a source of app membership. Core rows have no app tab/workspace
id and no authoritative app focus. Remote addresses remain qualified.

The list supplies no session IDs or PIDs. Local ownership proof uses the exact
`hc get --json ADDRESS` object's `address`, `state: live` and
`foreground: {pid,name,argv,cwd}`. Every read removes `HC_TERMINAL_ID` from the
child environment so it does not identify a parent caller or mark a child's
news as seen. Reads use argv directly, bounded output and a deadline; no shell
or mutation command is used. A remote PID never proves local ownership.

Claude core SessionStart/subagent hook records keep their existing v1 fields and
append `host: herdr-core`, `terminal`, nullable legacy `pane_id`, and `harness:
claude`, beside the existing `pid`, `pstart` and `pidns` process identity.
The environment terminal is a claim. Only a root SessionStart record with an
exact terminal, matching current foreground PID/start token and any supplied
PID namespace, and recognized
Claude process selects a transcript session. Multiple session IDs proven for
the same terminal/process are ambiguous: no recency or cwd fallback is used.
Records without core metadata remain valid for the existing TUI; they cannot
acquire a core terminal. Malformed hooks remain benign and exit zero.

Codex core hooks reuse existing root lifecycle validation, embedded-process
ancestry proof, metadata-only root verification, private association Store and
fresh process ownership checks. Core filenames use a separate `core-` namespace
and contain explicit host/terminal alongside `pane_id: core:ADDRESS`, so even a
TUI pane literally named `core:ADDRESS` cannot overwrite a core binding.
Readers revalidate exact address/current foreground/start token and any supplied
PID namespace on every collect. Like the kit process guard, a recorded foreign
namespace cannot prove local ownership; absent namespace retains macOS behavior.
Conflicting automatic bindings remain unavailable; an operator can explicitly
choose `panorama codex-bind THREAD_ID --terminal ADDRESS` when needed. This
change does not run that command for existing sessions. Remote binding is refused.

Automatic linking from the foreground process (Claude per-process record, Codex
open rollout) and the hook ordering rule are specified in
[session-linking.md](session-linking.md).

Missing proof yields a plain supported core root and an availability error;
source failures are `errors[]` data. Existing live sessions may have no new
terminal-aware record or verified core binding. Their rich-session enrichment
has **not** been accepted or bootstrapped by this change. No Claude/Codex config,
hook installation, TUI plugin installation, global PATH or live app layout is
modified. A3 will consume the explicit observation address map for feed v1;
A2 does not implement the feed writer.
