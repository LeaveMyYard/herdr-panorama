# Core session linking: proof rules

Status: design for the session-linking identity slice. It was requested by main and is not
part of the uniform-tree plan. The operator chose "Both": link automatically from the
process, and use the Claude SessionStart hook as a stronger signal. This document extends
[core-host.md](core-host.md) and narrows one of its rules (§3 H), as stated there.

A herdr-app core root gets rich data (session, threads, subagents) only when Panorama can
name **the exact conversation that the terminal's current foreground agent process is
running**. This document lists:
- which evidence may name that conversation;
- how each piece of evidence is proved to belong to that process;
- when Panorama refuses.

A refused root stays a basic native root, and `errors[]` gets an entry:
`core:tN: session ownership unavailable`, or `core:tN: session evidence conflicts`.
A refused root is never attached to a guessed conversation.

## 1. What herdr itself offers (investigated first)

| Source | Session identity? | Use |
|---|---|---|
| TUI `herdr api snapshot` | yes: `agent_session.value` per agent | unchanged TUI path (DESIGN.md "Orchestrators") |
| `hc ls --json` (hc 0.1.17) | no: `{address,agent,cwd,state,status,turn,where}` | roots only |
| `hc get --json tN` (hc 0.1.17) | no. It adds `foreground: {pid,name,argv,cwd}`, and `records` keyed by Claude's in-process agent ids (`{app,state,title}`), with no session id | the process to prove against; `tests/fixtures/hc-get-foreground.json` |
| herdr-app 0.1.12 `herdr.d.ts` `agents()` / `AgentInfo` | no: address, pane, space/tab names, status, `app`, title, seq | not used for identity |

herdr-core exposes no per-terminal session, so the core path has to derive it from the
foreground process. None of these is ever evidence:
- titles and tab names;
- `cwd` and `where`;
- the newest transcript, or any other recency;
- the Codex session-tree id;
- argv (see §3, "Not evidence").

## 2. The process gate (process-derived evidence)

The gate applies to a row with local address `tN` and harness `claude`. A remote
`machine/tN` is refused before anything is read, because a remote PID proves nothing on
this machine. The gate passes only if all of these hold:

1. `hc get --json tN` reports `state: live` and `foreground.pid` = P, with 1 < P ≤ u32::MAX.
2. Panorama reads start token S1 from `/proc/P/stat` field 22, **before** it reads any
   evidence. This is the kit's `start_token`, the same wire token that hook records carry.
3. Panorama's uid owns `/proc/P`.
4. `/proc/P` is Claude by the kit's `first_claude` test.
5. Its `comm` equals hc's `foreground.name` exactly, and its NUL-split `cmdline` equals
   hc's `foreground.argv`, with empty arguments dropped on both sides (the kit drops them
   from `cmdline`). hc 0.1.17 fills `name` from `comm` and `argv` from `cmdline`; the
   recorded shape is in `tests/fixtures/hc-get-foreground.json`. Herdr-core terminal numbering is per instance. This check stops a PID
   from another PID namespace, or from a second core instance, from matching an unrelated
   local process that happens to sit in its own `tN`.
6. **Terminal environment.** `/proc/P/environ` contains `HC_TERMINAL_ID=tN` exactly once.
   - Only three keys are read: `HC_TERMINAL_ID`, `CLAUDE_CONFIG_DIR` and `HOME`. The buffer is
     bounded (256 KiB) and dropped at once. It is never logged, stored, or compared with
     anything else.
   - If the environment is larger than the bound, or unreadable, the gate refuses. That is
     the safe direction.
   - A process started in another terminal, or in a container, does not carry this value.
   - The process's effective config dir, `${CLAUDE_CONFIG_DIR:-$HOME/.claude}` from its own
     exec environment, must equal Panorama's, in either direction; otherwise, or when
     neither key is set, the gate refuses. Paths compare by component (a trailing slash is
     the same dir); symlinks are not resolved, so an alias refuses.
7. Panorama reads the evidence (§3, P).
8. Panorama reads start token S2 **after** the evidence. If S1 ≠ S2, or the process has
   vanished, the gate refuses (the PID was reused during the read).

## 3. Evidence kinds

### H: hook records (Claude, existing `panorama hook session-start`)
These are `<hooks dir>/<session>.jsonl` v1 `event: session` lines with `host: herdr-core`,
`terminal: tN`, `harness: claude`, `pid`, `pstart`, and an optional `pidns`, as today. A
record **matches** when all of these hold:
- its `pid` and `pstart` equal the current foreground P and S;
- its `pidns`, if supplied, equals Panorama's;
- the existing `owns` and `first_claude` checks pass (core-host.md).

How many sessions the matching records name decides H:
- **One session** names that session.
- **More than one session** is ambiguous, as core-host.md states. One exception: when P
  (below) is present and names the session of the newest matching record (by `ts`), the two
  agree, and that session links. The newest record on its own never decides. The hook drops
  a record it cannot write, so the newest surviving record can be stale.

Subagent `start`/`stop` records are **not** evidence. They say which session the process was
running when it last spawned a subagent. After a later in-process switch they are as stale
as argv.

### P: Claude Code's per-process session record
**V1: verified from the primary source.** Claude Code 2.1.318 is a single bundled
executable. The registration code read from its embedded JavaScript does the following:
- It writes `<config dir>/sessions/<pid>.json` with mode 0600, in a 0700 directory, as
  `{pid, sessionId, cwd, startedAt, procStart, version, kind, entrypoint, …}`.
- `procStart` is the `/proc/<pid>/stat` start field, the same token as S.
- It subscribes to the session-id change signal. That signal is emitted by `/clear` (new id)
  and by every session switch (`/resume`, continuation, adoption). On each change it rewrites
  the file through `updatePidFile` with `{sessionId: <new>, updatedAt}`.
- It removes the file on exit.

`<config dir>` is `${CLAUDE_CONFIG_DIR:-$HOME/.claude}` as Panorama resolves it, and §2.6
makes it equal to the process's own.

Anchors in 2.1.318, for re-checking a later version: registration runs only behind the
concurrent-sessions gate (`f_()`); when it is off there is no file and P is absent. The
rewrite is the subscriber on the session-id change signal (`liveSessionId`, then
`updatePidFile({sessionId, parkedJobId, updatedAt})`), and that signal is emitted by the
clear path and by the switch path, whose reasons include resume, fork, cd, remote_attach,
hydrate, spare_claim and startup_custom_id.

Panorama reads the file under these conditions:
- read-only, opened `O_NOFOLLOW`;
- a regular file owned by our uid, with `nlink == 1`, ≤ 16 KiB;
- a strict JSON object.

P names `sessionId` only when all of these hold:
- `pid` equals P;
- `procStart` is a string equal to S. This is the exact PID-reuse proof: a file left behind
  by an earlier process with the same PID has another token;
- `kind` is `"interactive"` (the other values, `bg`, `daemon` and `daemon-worker`, are not
  the plain terminal session this design reasons about);
- `sessionId` passes `SessionId::parse`.

Unknown fields are ignored. Panorama never logs or copies them. If the file is missing or
any check fails, P is *absent*. When Claude uses its alternative storage backend, the file
is not at this path, and P is absent. P covers Claude sessions that were already running
when Panorama started, with no hook configured.

Synthetic fixtures copy this shape exactly. If a later Claude version drops or renames
`procStart`, P becomes absent, which is the safe outcome.

### B: Codex association (existing A2, unchanged)
This is the private `core-…` association that `panorama codex-bind THREAD --terminal tN` or
the Codex hook writes. The existing `resolve_core` consumes it and checks the owner PID and
start token, the PID namespace, and foreground membership. A `conflicted` record refuses, as
it does today.

### Not evidence
**Codex open rollout (V2: verified negative).** In Codex 0.166.0,
`codex-rs/rollout/src/recorder.rs` creates a new thread's rollout with
`deferred_creation: true`. The file is opened on first materialisation, which is after the
first user message. So just after an in-process `/new`, the only open rollout can still be
the previous thread's. An open rollout therefore cannot prove the current thread, and it is
not used. Codex core roots link only through B. An embedded Codex that runs the Codex hook
gets B automatically. A shared-daemon TUI needs `codex-bind`.

**Argv.** Claude's `--resume ID` and `--session-id ID`, and `codex resume ID`, name only the
session at exec. An in-process switch changes the session without changing argv, so argv
can never be re-verified.

**Other Claude state.** Claude keeps no transcript open between writes (DESIGN.md). Its
child-only environment, such as `CLAUDE_CODE_SESSION_ID`, is not set in the Claude process's
own exec environment.

## 4. Decision per terminal

The signals are {H, P} for a Claude row and {B} for a Codex row. Each signal is *absent*,
*ambiguous*, or names one id.

1. All present signals name the same id: that id links. The link is written as
   `agent_session` `{"kind":"id","value":ID,"source":"panorama:core"}`, the existing seam
   into the collector.
2. H is ambiguous, but its newest record agrees with P: that id links (§3 H).
3. Any other ambiguous signal, or no signal at all: refuse, with "session ownership
   unavailable".
4. Present signals disagree: refuse, with "session evidence conflicts". Two consequences:
   - A Claude process whose hook record lags or leads its per-process record is refused
     until the two agree.
   - A continuation (`continued-in`) that the hook did not record also refuses. This is
     accepted: refusing is safe, and the hook records resumes and clears.

After all terminals are decided, two cross-terminal rules apply:
- If more than one terminal links the same session id, every one of them is refused. This
  covers a transcript shared by two processes.
- If more than one terminal reports the same foreground PID, every one of them is refused.
  This covers two terminals over one process tree, and `hc attach` of the same program.

Uniqueness is checked on the proved ids. The collector's existing `continued-in` following
is unchanged.

## 5. Re-verification and coverage

Automatic linking persists nothing. Every collect recomputes the gate and every signal, so
both kinds of change show up on the next collect:
- a foreground change: a new P or S after an exit, a resume started as a new process, or an
  `hc keep` replacement;
- an in-process switch: a rewritten P, or a new H record.

A stale link cannot outlive its process. Coverage:
- **Already-running Claude sessions** are covered by P on Linux, with no hook. The
  foreground PID must be the Claude executable itself, not a wrapper.
- **Codex sessions** are covered only by B (the Codex hook or `codex-bind`).
- **Other platforms** (no `/proc`): only H and B apply.

## 6. Threat cases

| Case | Outcome |
|---|---|
| PID reused between the hc read and the evidence read | S1 ≠ S2, or the process is no longer Claude → refuse |
| Per-process record left by a reused PID | `procStart` ≠ S → P absent |
| Hook records from an earlier process with the same PID | `pstart` differs → no match |
| Container or other PID namespace: hc's P is a different local process | comm/argv mismatch, no `HC_TERMINAL_ID=tN`, or a foreign `pidns` on H/B → refuse |
| Second herdr-core instance with the same `tN` numbering | comm/argv must equal hc's, and the start token must be stable; a different process fails |
| Remote terminal `machine/tN` | refused before any read |
| Process started in t1 that is now the foreground of t2 | `HC_TERMINAL_ID` mismatch → refuse |
| Two terminals over one process tree, or the same foreground PID | cross-terminal uniqueness → refuse both |
| Two Claude processes resumed into one session (shared transcript) | the same id links in two terminals → refuse both |
| In-process `/resume` or `/clear` | P is rewritten. Until the H record lands, or if it is lost, H and P disagree → refuse |
| Switch read before Claude's asynchronous record rewrite lands | P names the previous session for at most that window (milliseconds, on Claude's write chain). The next collect corrects it. With the hook configured, the new H record conflicts → refuse. Accepted and stated. |
| Codex in-process `/new` (deferred rollout) | rollouts are not evidence; B only |
| Claude's record rewrite fails after a switch (`updatePidFile` logs and returns, e.g. ENOSPC) | the old `sessionId` stays with a matching `procStart`. With P alone nothing contradicts it: **accepted residual**. With the hook configured, the new H record conflicts → refuse |
| A hook write is lost after a switch and only the older SessionStart record remains | H alone names the old session (the existing A1 behaviour): **accepted residual** where P is unavailable. Where P is present, it names the new one → conflict → refuse |
| Process with its own `CLAUDE_CONFIG_DIR` | it differs from Panorama's → refuse |
| Hook record forged by another local user | the hooks dir is per-user state, and the record must still match the live P, S and terminal; another uid's process fails the gate |
| Symlinked or hard-linked per-process record | `O_NOFOLLOW`, `nlink == 1` and the uid check → absent |
| Oversized or malformed evidence | bounded reads; malformed → absent |
| `/proc/P/environ` unreadable (other uid, hardened `/proc`) | refuse |

## 7. The hook as an operator setup

`panorama hook session-start` is kept. `panorama hook setup` prints the `hooks` JSON to merge
into Claude Code's user settings (`~/.claude/settings.json`, or
`$CLAUDE_CONFIG_DIR/settings.json`). It never writes that file. The printed JSON contains:
- the running binary's absolute path, quoted for `sh`;
- the SessionStart matcher used by the repository's `hooks/hooks.json`;
- the existing SubagentStart and SubagentStop entries.

The operator applies it. Panorama never edits `~/.claude` or `~/.codex`.

## 8. Code placement

- **`src/host/link/` (new):** the gate, P, the decision, and cross-terminal uniqueness. Each
  module stays under 300 non-test lines.
  - Readers are injected: a `/proc` root path, a Claude config dir, and the kit's
    `ProcessTable`. Tests therefore use synthetic directories and `FakeProcesses`, never the
    operator's processes.
- **`records.rs`:** returns H as a signal that keeps the newest session, for §4.2.
- **`source.rs`:** calls the decision at the point where it calls
  `claude_session`/`resolve_core` today.

Codex identity code, the feed v2 serializer and app mods are not touched. The result flows
through the existing `agent_session` row field.
