# Local Codex integration

Status: approved scope; implementation in progress on `feature/codex-integration`.
Baseline: `origin/main` at `b2ec30d38bffb578661348d2ab5d976ac825bf84` (2026-11-02).

## Goal and scope

Existing and newly launched Claude Code and Codex CLI panes share Panorama's left
panel, with correct navigation, lifecycle, and Codex session details when an exact
session association is available. Implement this in Rust, preserving the Python
feature freeze and the harness-neutral boundaries in
[the Rust rewrite plan](rust-rewrite.md#harness-neutral-seams).

The approved local scope includes mixed pane listing and panels, Codex launching,
exact pane/thread registration, history, turns, tools, child agents, status, and
optional `report_thread` MCP support. Detailed remote Codex tracking and inbox
answer delivery are follow-ups. This work does not activate the live plugin or
write the operator's Codex configuration or authentication state.

## Observable behavior

- Claude and Codex panes remain visible and visibly identify their harness.
- Arrow navigation, mouse selection, and Enter focus the correct existing pane,
  retaining current filters, tabs, and panel behavior.
- The existing new-agent form enables Codex using herdr's `agent.start --kind
  codex` capability. Launch success gets the correct panel and tab; failed starts
  use safe, narrowly scoped cleanup.
- Exact Codex identity enriches a pane with its thread's history, turns, tools,
  child agents, and supported status observations.
- Missing hooks, an unavailable history store, unsupported versions, or an
  unavailable app-server leave the pane visible with trustworthy herdr state and
  unavailable detail. They neither hide the pane nor stall collection or the UI.
- Claude behavior remains covered by the existing regression suite. Additive
  model and wire fields have compatible defaults for older state and peers.

## Identity and lifecycle

Use herdr pane IDs and `pane.report_agent_session` for lifecycle registration.
Codex registration must not depend on Claude hooks. Inspect supported Codex hook
payloads and lifecycle behavior before implementing the bridge; preserve hook
stdin/output semantics and do not inject model instructions for telemetry.

Associate a pane only with an exact thread identity. Never choose a session by
the newest file, directory similarity, or shared working directory: concurrent
sessions in one directory are normal. An unbound pane remains useful and visible.
Resume and fork must refresh the correct association without confusing the old
and new threads. Codex session-tree group `sessionId` and individual thread `id`
can differ; root and child handling must validate the relevant identity.

A child hook must not overwrite the pane's root identity. Child sessions belong
under their actual parent and may not become unrelated root sessions merely
because they share a pane or directory. Lifecycle registration and read-only
display observations are separate responsibilities.

## Read-only Codex observations

Build a version-aware adapter that emits neutral observations for the existing
thread and agent model. Codex parsing, paths, and transport details stay behind
that adapter rather than leaking Claude-specific abstractions into consumers.

The design must accommodate both legacy JSONL and paginated history. Prefer the
app-server read operations `thread/read`, `thread/turns/list`, and
`thread/items/list` over a private SQLite schema. Validate the installed protocol
and available operations rather than assuming version support. Honor
`CODEX_HOME`, including isolated custom homes.

All I/O, pagination, caching, retries, and failure handling must be bounded. A
fresh observer app-server does not own another server's runtime state, so its
runtime status must not be represented as the original session's live status.
Represent active, blocked, failed, completed, and unknown only from evidence with
the appropriate meaning. Idle or `notLoaded` alone never proves completion.

Expose tools, turns, and child agents without logging raw transcripts or secrets.
Use synthetic fixtures for test evidence, screenshots, and review artifacts.

## Optional MCP reporting

Keep `report_thread` opt-in. Resolve reports to the exact Codex session and
document registration and setup commands, supported versions, and limitations for
already-running sessions. Do not silently edit `~/.codex/config.toml`, read or copy
credentials, or imply that optional MCP is required for basic pane visibility.
Inbox answer delivery remains outside this change.

## Implementation slices and ownership

| Slice | Responsibility | Dependencies |
|---|---|---|
| Bootstrap | Dedicated integration worktree, this plan, draft umbrella PR | Fresh `origin/main` |
| Visibility and launcher | Additive harness identity; wire/render paths; mixed collector filtering and panel management; Codex launch; navigation regressions | Bootstrap |
| Codex lifecycle | Exact root identity registration and safe child handling; supported hook CLI | Bootstrap; agreed model contract |
| Codex observation | Read-only version-aware adapter and bounded neutral history/status output | Bootstrap; agreed observation contract |
| Optional reporting | Codex-aware MCP/emit identity resolution and setup guidance | Lifecycle identity contract |
| Integration and verification | Final collector dispatch/enrichment, neutral model mapping, end-to-end fixtures, documentation, full gates | Relevant prerequisite slices merged |

Each independent responsibility gets a small PR targeting
`feature/codex-integration`. Visibility owns model/view/new-agent and initial
collector filtering. The Codex lane owns adapter modules and hook/MCP/emit paths.
Integration owns final collector enrichment after prerequisites merge, integration
fixes, and complete verification. Cross-file contracts are agreed before edits.

## Acceptance and evidence

| Area | Required evidence |
|---|---|
| Mixed visibility | Existing Claude and Codex panes appear with harness identity, including unbound Codex and unavailable details |
| Navigation and panels | Arrows, clicks, and Enter target the right pane; tab/panel behavior and filters remain correct |
| Launch | Correct Codex kind and panel/tab; failed launch cleanup never closes unrelated panes |
| Identity | Same-directory concurrency, resume, fork, group/thread ID differences, and child hooks preserve exact root binding |
| History | Legacy and paginated fixtures, custom `CODEX_HOME`, bounded reads, tools and turns, children nested under the right parent |
| Status | Evidence-driven active/blocked/failed/completed/unknown; idle and `notLoaded` do not imply completion |
| Degradation | Missing hooks/store/server and unsupported responses remain nonfatal and bounded |
| Compatibility | Old/new state and wire compatibility; existing Claude regressions pass |
| Optional reporting | Exact Codex identity resolution, explicit opt-in setup, documented existing-session limits |
| Whole integration | Durable synthetic end-to-end coverage and isolated changed binary; appropriate render evidence; full local and CI gates |

Tests use `FakeHerdr` or a separate isolated instance and private synthetic state,
cache, and databases. Unset every `HERDR_*` variable before every build or test.
Use a private `CARGO_TARGET_DIR` per worktree, `nice -n 10`, and at most two Cargo
jobs for the normal verification runs. Do not call live mutation APIs, rebuild or
restart the live plugin, inspect user auth, or kill operator processes.

Each code PR gets independent review and real, successful, non-skipped
`pull_request` CI on its exact reviewed head before a merge commit into the
integration branch. Review-cycle modes are `auto / github`; identity association
is safety-critical and receives a fresh final independent review before merge.

Final verification runs Rust formatting, Clippy, workspace tests, the Python
suite, structure checks, and the Python test ledger. Rust files stay within 300
non-test lines with tests in separate files. macOS validation uses an installed
target when available or CI. Full gates are recorded with their exact SHA and
limitations; live acceptance is never inferred from synthetic tests.

The umbrella PR remains draft until a fresh independent full-effort review and
the complete local and CI gates succeed. It then becomes ready for the operator;
this effort does not merge the umbrella PR or deploy the feature.
