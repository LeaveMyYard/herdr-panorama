# Core session linking: proof rules

Status: design for the session-linking identity slice (main's request; operator decision
"Both": automatic linking from the process, plus the Claude SessionStart hook as a stronger
signal). Extends [core-host.md](core-host.md); nothing here weakens a rule stated there.

A herdr-app core root gets rich data (session, threads, subagents) only when Panorama can
name the **exact conversation the terminal's current foreground agent process is running**.
This document lists the evidence that may name it, how each piece is proved to belong to
that process, and when Panorama refuses. A refused root stays a basic native root with a
`core:tN: session ownership unavailable` (or `... session evidence conflicts`) entry in
`errors[]`; it is never attached to a guessed conversation.

## 1. What herdr itself offers (investigated first)

| Source | Session identity? | Use |
|---|---|---|
| TUI `herdr api snapshot` | yes: `agent_session.value` per agent | unchanged TUI path (DESIGN.md "Orchestrators") |
| `hc ls --json` (hc 0.1.17) | no: `{address,agent,cwd,state,status,turn,where}` | roots only |
| `hc get --json tN` | no: adds `foreground: {pid,name,argv,cwd}` | the process to prove against |
| herdr-app 0.1.12 `herdr.d.ts` `agents()` / `AgentInfo` | no: address, pane, space/tab names, status, `app`, title, seq | not used for identity |

herdr-core exposes no per-terminal session, so the core path must derive it from the
foreground process. Titles, tab names, `cwd`, `where`, newest transcript, recency and the
Codex session-tree id are **never** evidence.

## 2. The process gate (every automatic kind)

For a row with local address `tN` (a remote `machine/tN` is refused: a remote PID proves
nothing here) and harness `claude` or `codex`:

1. `hc get --json tN` is `state: live` with `foreground.pid` = P, 1 < P ≤ u32::MAX.
2. Start token S1 = `/proc/P/stat` field 22 (the kit's `start_token`, the same wire token the
   hook records carry), read **before** any evidence.
3. `/proc/P` is owned by Panorama's uid and is the row's harness: `comm`/`argv[0]` basename
   `claude` for Claude (kit `first_claude`), `codex` (not `app-server`/`daemon`) for Codex.
4. **Terminal environment**: `/proc/P/environ` has `HC_TERMINAL_ID=tN` exactly. Only that key
   is extracted; the buffer is bounded (64 KiB), dropped immediately, never logged, stored or
   compared to anything else. A missing, unreadable or different value refuses. This is what
   ties the PID in Panorama's `/proc` to that core terminal: a PID from another PID
   namespace or container, or a process started in another terminal, does not carry it.
5. Evidence (§3) is read.
6. Start token S2 is read again **after** the evidence; S1 ≠ S2, or a vanished process,
   refuses (PID reuse during the read).

The gate proves "the process in Panorama's `/proc` at P is the one hc reports as tN's
foreground and was started inside tN". The evidence then proves which conversation it runs.

## 3. Evidence kinds

### H — hook records (Claude; existing `panorama hook`)
`<hooks dir>/<session>.jsonl` v1 lines with `host: herdr-core`, `terminal: tN`,
`harness: claude`, `pid`, `pstart` and (optional) `pidns`, written by the hook under the
actual Claude process. A record **matches** when `pid`/`pstart` equal the current foreground
P/S, its `pidns` (if supplied) equals Panorama's, and the existing `owns`/`first_claude`
checks pass (core-host.md).

- `event: session` records (SessionStart: startup, resume, clear, compact, fork) decide: of
  the matching session records, the one with the greatest `ts` names the current session.
  SessionStart fires on every in-process switch, so the newest one from the **same process**
  is that process's own latest statement, not a recency guess across processes. Two
  different sessions with the same greatest `ts` are ambiguous.
- Only when no matching session record exists, matching `start`/`stop` (subagent) records
  may decide, and only if they all name one session (their file is the root session the
  payload named). This covers a process that was already running when the hooks were added
  and has used subagents since. Different sessions among them are ambiguous.

H is the **stronger** signal: it is produced by the harness at the switch itself.

### P — Claude per-process session record
`${CLAUDE_CONFIG_DIR:-$HOME/.claude}/sessions/<P>.json`, the harness's own record of the
session that process is running. Read-only, opened `O_NOFOLLOW`, regular file, our uid,
`nlink == 1`, ≤ 16 KiB, strict JSON object. Accepted only when:
- `pid` equals P, `sessionId` is a canonical session id (kit `SessionId::parse`);
- `startedAt` (ms since epoch) is present and within 10 s of P's start time
  (`btime + starttime / USER_HZ`, Linux; elsewhere P is unavailable). A record left behind by
  an earlier process that had the same PID fails this (PID reuse);
- the file's mtime is not before P's start time.
Unknown extra fields are ignored; any of the above missing or malformed refuses P. P covers
sessions already running when Panorama starts, with no hook configured.
*Verification item V1:* the field names and the claim that Claude Code rewrites this record
when the same process switches session (`/resume`, `/clear`) must be confirmed read-only on
the live install by someone authorised to read `~/.claude/sessions`; until then P is the
documented assumption and the conflict rule (§4) is the guard when H is present.

### B — Codex association (existing A2)
The private `core-…` association in `$XDG_STATE_HOME/herdr-panorama/codex-bindings`, from
`panorama codex-bind THREAD --terminal tN` or the Codex hook, consumed by the existing
`resolve_core` (owner PID/start, namespace, foreground membership). A `conflicted` record is
an **ambiguous** signal (it refuses, as today).

### F — Codex open rollout
The links in `/proc/P/fd/*` whose target is a regular file under `$CODEX_HOME/sessions/`
named `rollout-<timestamp>-<thread>.jsonl` with a canonical thread UUID, and whose
`/proc/P/fdinfo/<fd>` flags are open for writing (`O_WRONLY`/`O_RDWR`). An embedded
(`--no-daemon`) Codex keeps the rollout of every thread it runs open for appending, so a
writable open rollout is that process's own record. Subagent threads run in the same process
and have their own rollouts, so each candidate goes through the existing A2 metadata
verifier (`root::verify`: `thread/read`, `parentThreadId: null`, root source). Exactly one
verified root among them names the session; zero or more than one refuses. A shared-daemon
Codex TUI holds no rollout (the daemon does, for many terminals), so it has no F and stays
basic unless bound with `codex-bind` (documented, A2 unchanged).
Verification results are cached per (P, S, thread): a proven root is not re-verified while
that process lives; a failed verification is retried at most every 30 s; at most one
verifier spawn per collect, within the collect's existing deadline.

### Not evidence
- Claude `argv` `--resume ID` / `--session-id ID`: names only the session at exec. An
  in-process `/resume` or `/clear` changes the session without changing argv, so it can never
  be re-verified and is not used. Same for `codex resume ID`.
- Claude open files: Claude keeps no transcript open between writes (DESIGN.md).
- Claude child environment (`CLAUDE_CODE_SESSION_ID` is set for tool children, not in the
  Claude process's own exec environment) and any other environment value except §2.4.

## 4. Decision per terminal

Collect the signals for the row's harness: Claude {H, P}, Codex {B, F}. Each is *absent*,
*ambiguous*, or names one id.

1. Any ambiguous signal → refuse ("session ownership unavailable").
2. No signal → refuse (basic root).
3. All present signals name the same id → link that id (`agent_session`
   `{"kind":"id","value":ID,"source":"panorama:core"}`, the existing seam into the collector).
4. Present signals disagree → refuse ("session evidence conflicts"). In particular a hook
   record and the per-process record must agree; a manual `codex-bind` and the open rollout
   must agree (rebind after an in-process switch, as app/README says).

Then across terminals: a session id linked by more than one terminal, or a foreground PID
reported for more than one terminal, refuses every such terminal (shared transcript, two
terminals over one process tree, `hc attach` of the same program).

## 5. Re-verification and coverage

Nothing is persisted by automatic linking: every collect recomputes the gate and every
signal, so a foreground change (new P or S: exit, resume as a new process, `hc keep`
replacement) or an in-process switch (new H record, rewritten P, new open rollout) is seen on
the next collect, and a stale link cannot survive its process. Only the root-verification
cache (keyed by P and S) outlives a collect. Sessions already running when Panorama starts
are covered by P (Claude) and F (embedded Codex) without any hook or rebinding.

## 6. Threat cases

| Case | Outcome |
|---|---|
| PID reuse between hc read and evidence | S1/S2 differ or no longer the harness → refuse |
| Old per-process record for a reused PID | `startedAt`/mtime before the new process → P refused |
| Hook records from an earlier process with the same PID | `pstart` differs → not matching |
| Container / other PID namespace: hc's P is a different process here | no `HC_TERMINAL_ID=tN` (§2.4), or foreign `pidns` on H/B → refuse |
| Remote terminal `machine/tN` | refused before any read |
| Process started in t1, foreground of t2 | `HC_TERMINAL_ID` mismatch → refuse |
| Two terminals, one process tree / same foreground PID | cross-terminal uniqueness → refuse both |
| Two Claude processes resumed into one session (shared transcript) | same id for two terminals → refuse both |
| In-process `/resume` / `/clear` | H newest SessionStart and/or rewritten P; a stale P disagrees with H → refuse |
| Codex subagent rollouts open in the root's process | metadata verifier keeps roots only; >1 root → refuse |
| Shared-daemon Codex TUI | no F; only an explicit `codex-bind` (B) |
| Hook record forged by another local user | hooks dir is per-user state; the record must still match the live P/S and terminal; another uid's process fails the gate |
| Symlinked / hard-linked per-process record | `O_NOFOLLOW`, `nlink == 1`, uid check → refuse |
| Oversized or malformed evidence | bounded reads; malformed → that signal absent/refused |
| Panorama cannot read `/proc/P/environ` (other uid, hardened `/proc`) | refuse |

## 7. The hook as an operator setup

`panorama hook session-start` is kept. `panorama hook setup` prints, and never writes, the
exact `hooks` JSON to merge into Claude Code's user settings (`~/.claude/settings.json`), with
the absolute path of the running binary and the SessionStart matcher
`startup|resume|clear|compact|fork`, plus the existing SubagentStart/SubagentStop entries.
The operator applies it; Panorama never edits `~/.claude` or `~/.codex`.

## 8. Code placement

`src/host/link/` (new, < 300 non-test lines per module): the gate, P, F, the decision, and
cross-terminal uniqueness, behind injected readers (a `/proc` root path and a Claude config
dir) so tests use synthetic directory fixtures and the kit's `FakeProcesses`, never the
operator's processes. `records.rs` gains the H ordering rule. `source.rs` calls the decision
where it calls `claude_session`/`resolve_core` today. Codex F reuses `codex::identity` root
verification through a small crate-visible wrapper. The feed v2 serializer and app mods are
not touched; the result flows through the existing `agent_session` row field.
