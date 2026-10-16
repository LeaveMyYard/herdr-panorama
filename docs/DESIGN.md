# herdr-panorama: design (PoC)

## Problem

The operator runs several Claude Code sessions in herdr tabs. Each one
orchestrates work: it spawns subagents (authors, reviewers, explorers), resumes
them with `SendMessage`, runs native Workflows, and sometimes stops on a
permission prompt or a question. Nothing shows that structure. To learn what a
tab is doing, the operator has to open it and scroll.

herdr-panorama is a read-only swarm view of that structure, drawn like an Argo resource
graph: **orchestrators → threads → agents**, with small coloured squares
("pods") as the status mark. It implements variant **2a "Graph · squares are
subagents"** of the Claude Design project (copies in `docs/design/`, rendered
reference in `docs/design/2a.png`).

Scope of the PoC: Claude Code sessions only, running in herdr panes. The view
never types into a pane. Its only actions are focusing a pane (`g`) and
opening the herdr-inbox popup (`a` / `i`) when that plugin is installed.

## Components

```
herdr api snapshot ──┐
                     │      ┌────────────┐      ┌─────────────────────┐
~/.claude/projects/  ├────▶ │ collect.py │ ───▶ │ model.Swarm (JSON)  │──▶ panorama snapshot --json
  <slug>/<sid>.jsonl │      │ (cached,   │      └─────────────────────┘
  <sid>/subagents/…  │      │  tail-read)│                │
  <sid>/workflows/…  │      └────────────┘                ▼
inbox.db (ro, opt.) ─┤                              tui.SwarmApp (Textual, cell grid)
events/<sid>.jsonl ──┘ (explicit thread channel)          │
                                                          └─▶ herdr agent focus <pane>
                                                              herdr plugin action invoke …inbox.open
```

One package `herdr_panorama`, CLI `panorama`:

| Command | Purpose |
|---|---|
| `panorama tui` | the Textual view (herdr popup or split) |
| `panorama snapshot [--json] [--counts]` | print the derived model; `--counts` prints numbers only, no content |
| `panorama emit …` | append a thread declaration to the explicit channel (see below) |
| `panorama open [--split]` | open the plugin pane (used by the herdr actions) |

Runtime: Python ≥ 3.11, uv, Textual. Storage: none of its own, apart from the
optional event files the orchestrators write.

## Data model

`model.py` holds plain dataclasses; `panorama snapshot --json` prints them.

- **Swarm**: `orchestrators`, `generated_at`, counters `questions`, `blocked`,
  `failed`, `running`.
- **Orchestrator**: one per herdr pane whose agent is `claude`. `pane_id`,
  `tab_id`, `tab_label` (the name the operator gave the tab), `workspace_id`,
  `session_id`, `herdr_status` (`working` / `idle` / `blocked` / `done` /
  `unknown`), `flavour`, `transcript` (path), `pending` (what a `blocked` pane
  waits on: AskUserQuestion text or "wants Bash(…)"), `inbox` (open herdr-inbox
  question ids), `threads`.
- **Thread**: `id`, `name`, `source` (`explicit` / `workflow` / `ref` /
  `turn`), `phases`, `agents`, `status` (`active` / `done` / `failed`),
  `started`, `ended`, `worktree`, `pct`.
- **Phase**: `name`, `state` (`done` / `now` / `next` / `failed` /
  `skipped`), `start`, `end`, `note`.
- **Agent**: `id` (Claude agent id), `tool_use_id`, `description`, `role`,
  `agent_type`, `model`, `status`, `started`, `ended`, `last_at`,
  `tool_calls`, `activity` (last tool call, one line), `last_message` (last
  assistant text, one paragraph), `questions` (open inbox questions the agent
  filed), `error`, `phases` (indexes into the thread's phases), `parent`
  (spawning agent id for nested agents), `depth`, `resumes`.

Agent `status` uses the design's eight codes: `q` queued ○, `run` running ⠋
(spinner ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏), `rev` reviewing », `wait` needs answer ?, `blk` blocked !,
`fail` failed ✗, `done` ✓, `cxl` cancelled -. Amber is reserved for `wait` and
`blk`, red for `fail`. Turn 8 adds `bg` waiting ⠁ (lavender, one braille dot circling the cell,
`⠁⠈⠐⠠⢀⡀⠄⠂`): see *Waiting and stale*. Agents and orchestrators carry `bg` (their background
tasks in flight); orchestrators carry `last_activity`; the swarm carries `stale_after`.

## Data layer (`src/herdr_panorama/scan/`)

The scanner is a Python port of [zoetrope](https://github.com/furkankly/zoetrope)
v0.2.0 (MIT): its discovery, linking, lenient record parsing and append-only
tailing. Each module names the zoetrope source it follows. The
study it is based on compared zoetrope with real sessions on this machine and
found three classes of status error, which the port fixes (below). Verified on
Claude Code 2.1.x transcripts and herdr 0.9.3, read-only.

### Orchestrators: `herdr api snapshot`

One call returns `agents`, `panes`, `tabs` and `workspaces` (a few ms). For
each agent with `agent == "claude"` and `agent_session.kind == "id"`:

| Field | Source |
|---|---|
| `pane_id`, `tab_id`, `workspace_id`, `focused` | agent entry |
| `herdr_status` | `agent_status` (`idle` / `working` / `blocked` / `done` / `unknown`) |
| `session_id` | `agent_session.value` |
| `tab_label` | the tab's `label`; without one, the session's `custom-title`, `agent-name` or `ai-title` record, then the terminal title |

Orchestrators follow herdr's tab order; several Claude panes in one tab are
labelled `tab · pane`.

### Discovery (zoetrope `discovery.rs`)

- Project dir: the cwd with every non-alphanumeric character replaced by `-`
  (`/home/x/.cfg` → `-home-x--cfg`), under `${CLAUDE_CONFIG_DIR:-~/.claude}/projects`.
  If `<session>.jsonl` is not there (Claude Code shortens very long project
  paths), every project dir is searched once.
- A session's files: `<session>.jsonl` (root), `<session>/subagents/agent-<id>.jsonl`
  + `.meta.json`, and per workflow run `<session>/subagents/workflows/<runId>/`
  with `journal.jsonl` and its own agent files. The run summary is
  `<session>/workflows/<runId>.json`. Directories that do not exist yet are
  empty, never errors. The listing is re-scanned every tick.

### Tailing (zoetrope `tailer/bytes.rs`)

Per file: a byte offset and a partial-line buffer. Each tick stats the file and
reads only the appended bytes; complete lines go on, a trailing partial line
waits for its newline. A file that shrank or was replaced (`(dev, ino)`
changed) is read again from 0 and everything derived from the session is
rebuilt. A partial line over 8 MB is dropped and reading resyncs at the next
newline. CRLF is tolerated.

### Records (zoetrope `wire.rs`, `mod.rs`)

Every field is optional; unknown record types and blocks are skipped;
`message.content` may be a string or a list and `toolUseResult` a string or an
object; a line that does not parse states nothing. A record without a
timestamp inherits the previous one in its file. What is read:

| Record | Taken |
|---|---|
| `assistant` `tool_use` `Agent`/`Task` | a spawn: call id, owner (main or the agent whose file it is in), description, model, `subagent_type`, `run_in_background` |
| `user` `tool_result` + `toolUseResult` | spawn acknowledgement (`agentId`, `status: async_launched|completed`, `resolvedModel`, `totalToolUseCount`, `is_error`); workflow launch (`runId`, `taskId`, `workflowName`, `summary`); resume (`resumedAgentId`, unless `success` is false); herdr-inbox receipt (`Recorded as Q12`) |
| `<task-notification>` blocks (every block: one carrier can hold several) in a `user` string, a `queue-operation` `enqueue` (`content`) or an `attachment` `queued_command` (`prompt`), **in any file** | `task-id`, `status` (`completed` → done, `failed`/`error` → fail, `killed`/`stopped` → cancelled), `usage.tool_uses` |
| `user` string with `origin.kind == "human"` (legacy: any string that is not a notice) | an operator prompt, for prompt eras |
| `custom-title`, `agent-name`, `ai-title` | session titles |
| journal `started {agentId, label, phase}`, `result {agentId}`, `failed {agentId}` | workflow membership, phase, completion, failure |
| `meta.json` | `agentType`, `description`, `model`, `toolUseId`, `spawnDepth`, `workflowPhase`, `requestShape` (re-read until it parses: it can be caught mid-write) |

Subagent files carry notices for the agents they spawned only as
`queued_command` attachments (3 493 seen, no queue-operations).

Per agent, from the end of its file (re-read only when it grew; the window
starts at 64 KB and doubles up to 8 MB until it holds at least two dated
records, so one huge last record cannot hide the timestamp or the call in
flight): the
last timestamp, the calls without a result (in flight), the current activity
(the last such call, else the last call: `Bash(description)`,
`Read(src/a.py)` relative to the cwd), the last assistant text, the model.
Tool calls: a byte count of `"type":"tool_use"` in the agent's assistant
records, or the notice's `tool_uses` when higher. A record that cannot be
applied is counted and skipped, never stopping the rest of the batch; the
count shows in the header's `refresh failed` chip.

### Linking

- An agent's spawning call is `meta.toolUseId` (else the acknowledgement that
  named its `agentId`). The call's owner is its parent, so a depth-2 agent
  nests under the agent that spawned it. A fork's file repeats its parent's
  history, including the call that spawned the fork, so the agent's own copy
  never counts. Of the other files holding the call, main wins, then the
  earliest. If only the agent's own copy was read (the parent's file can be
  skipped as old), `meta.parentAgentId` names the parent. Checked on four sessions:
  every agent with `spawnDepth ≥ 2` gets a non-main parent, and every agent at
  depth 1 gets main.
- Workflow agents belong to their run (the directory), with the phase from
  the journal, else `meta.workflowPhase`.

### Status (fixing zoetrope's three errors)

Folded into keyed stores (spawns, acknowledgements, notices, resumes, journal
results), so the result does not depend on the order files or lines are read
(tested by shuffling). Statuses are derived at query time:

1. **Terminal evidence**, latest first: the newest notice for the agent from
   any carrier in any file; a foreground spawn's `completed` acknowledgement
   (`is_error` → fail); a journal `result` (done) or `failed` (fail), both
   final; for a workflow agent with neither, the end of its run (the latest
   launch's notice) → cancelled.
2. **Resumes reopen**: a `SendMessage` result with `resumedAgentId` after the
   evidence, or the agent's own activity more than 5 s after it, makes the
   agent running again; the latest event wins (`completed` → resumed →
   `failed` → resumed → `completed` ends done). zoetrope pins the first
   ending.
3. **An awaited agent is running**: a foreground agent whose spawning call
   has no result yet, or a workflow agent without a journal result while its
   run is live. Its caller is still waiting on it, which is ground truth
   however long the agent stays silent (capped at 1 h of silence, like an
   abandoned call). A workflow agent that started before its run's latest
   launch belongs to an instance that was relaunched (resumed): the new
   instance re-runs unfinished steps with new agents, so the old one is
   cancelled.
4. **Liveness** without standing evidence: running if the agent wrote in the
   last 120 s, or has a call in flight for at most 1 h (an abandoned call
   expires; zoetrope keeps such agents running forever), or is a background
   agent not yet notified and quiet for at most 10 min (a long model turn
   writes nothing). Otherwise it ended silently: done, marked `inferred`.

An agent that ended its turn with no evidence but has a running descendant
is waiting on it, and counts as running. Its own notice only comes once no
background child is live.

Display refinements: a running agent with an open inbox question is `wait`;
a running reviewer is `rev`; a running agent silent for 30 min reads
`quiet 42m`. An ending inferred from silence is shown as **`done?`** (a muted
`✓?` pod and its own legend entry), never as a confirmed `done` (issue #2).
With the opt-in hook, an agent can also be **`lost`** (`~`); see below.

### Accuracy (the gate)

`tools/replay_oracle.py` is a hindsight oracle. It does not know the scanner's
rules; it only knows what happened next.
- **Replay:** for each session, every file is truncated at past instants T.
  There are 24 instants spread over the session and 24 inside activity gaps
  (a random agent write plus 2 to 30 minutes).
- **Production path:** the scanner runs on the copy with `now = T` and
  `Session.update(since=T - 1 h - 600 s)`.
- **Truth:** an agent was running at T if it wrote again, or got a notice,
  within 30 minutes after T, with no terminal notice, completed
  acknowledgement or journal result between its last write and T.
- **Parentage:** compared against the owner of the call named by
  `meta.toolUseId` in the full files.
- **Not scored:** instants in the last 30 minutes of the data, which have no
  future yet.

It prints counts only. Run on all 38 local sessions that have subagents, at
12 spread and 12 gap instants each (`--instants 12 --newest 40`), about 25k
agent-instants, about 850 of them really running:

| | false done | false running | parent wrong |
|---|---|---|---|
| after the first review (`35161e2`) | 21 | 8 | 38 of 22 330 |
| this version | 15 | 11 | 0 of 22 439 |

What the remaining misses are:
- **False done (15):**
  - 11 are agents resumed 2–6 minutes after T. The oracle counts the later
    resume as one continuous run; at T they had finished.
  - 3 got a `killed` notice after 83 minutes of silence, when their session
    ended.
  - 1 is a real miss: a background agent silent for 14 minutes, beyond the
    10-minute quiet window, that was then notified.
- **False running (11):** all inside the documented windows.
  - 4 instants of one agent waiting on its live children, which by design
    counts as running. The oracle disagrees only because it woke 40–80 minutes
    later.
  - 5 instants of background agents inside the 10-minute quiet window.
  - 2 instants inside the 120 s activity window.

The fixed classes were:
- forks of a subagent drawn under main (all 38 wrong parents);
- agents waiting on running children shown as done;
- a workflow agent the journal marked `failed`;
- foreground and workflow agents silent for more than 120 s while their
  caller still waited on them;
- agents of a relaunched run's earlier instance.

`tools/check_status.py` is kept as a rule-level cross-check only: it applies
the same rules, so it catches implementation slips, not wrong rules.

### Blocking questions and permission prompts

herdr reports `agent_status: "blocked"` when Claude Code shows a permission
prompt or an AskUserQuestion dialog; permission prompts are not in the
transcript. The orchestrator's in-flight calls (last 64 KB) tell which:

- `AskUserQuestion`: input `{"questions":[{"question","header","multiSelect",
  "options":[{"label","description"}]}]}` (captured). Its question texts are
  shown.
- anything else: `wants Bash(<description>)` (permission prompt).

Background subagents run with `requestNonInteractive: true` and never prompt,
so `!` belongs to orchestrators. The view only shows the text and focuses the
pane (`g`); it never allows, denies or answers.

### herdr-inbox (optional)

If `${HERDR_INBOX_DB:-${XDG_STATE_HOME:-~/.local/state}/herdr-inbox/inbox.db}`
exists it is opened with `file:…?mode=ro` (URI, read-only, 1 s busy timeout).
Only columns of the 0.1 `questions` table are read (`id`, `session_id`,
`pane_id`, `asked_by`, `question`, `status`, `created_at`), so every inbox
version works. Open = `status = 'open'`.

- orchestrator `?N` = open questions of its `session_id` (fallback `pane_id`);
- agent `?N` = open questions whose receipts are in the agent's own file; the
  rest go on a "main session" card;
- `a` (or a click on an agent card's `a answer` or an orchestrator's `?N`)
  opens the inbox **on that question group**, using herdr-inbox's deep-link API
  (≥ 0.6.0):
  - an agent opens with `inbox open --question Q<n>` (its first open question's
    group);
  - an orchestrator opens with `inbox open --pane <pane_id>` (its oldest open
    group);
  - the command runs as `uv run --quiet --frozen --no-dev --project <plugin_root> inbox …`,
    where `plugin_root` and `version` come from `herdr plugin list --json`. It
    gets the inbox's own `HERDR_PLUGIN_ID` / `HERDR_PLUGIN_ROOT`; panorama's
    would open panorama's pane.
  - Support is the version (≥ 0.6.0) or `inbox open --help` mentioning
    `--pane`.
  - The call runs without panorama's own invocation context
    (`HERDR_PLUGIN_CONTEXT_JSON`, `HERDR_PANE_ID`, the plugin state and config
    dirs), so nothing of panorama's leaks into the inbox.
  - An older inbox falls back to `herdr plugin action invoke
    leavemyyard.herdr-inbox.open`.
  - A failed deep link falls back too, but only when it certainly failed
    before any popup opened: the inbox reported that it could not open the
    popup, uv or argparse failed, or it could not be spawned. Any other
    failure, including a timeout, opens nothing more, so there is never a
    second popup.
  - "Not installed" is looked up again after 60 s; a found inbox is kept.
  - `--no-hold` (herdr-inbox >= 0.6.1) asks for a normal popup rather than one held open.
    Panorama opens the inbox only on an explicit `a` or click, so it passes the flag, but
    only when `inbox open --help` lists it. The help is probed once per inbox version; an
    older inbox, or a failed probe, gets the command without it.
  - `i` always opens the plain inbox.
  - In a popup, panorama closes first (popups are modal), then opens the inbox
    within the inbox's 5-second request window. In the home tab the inbox
    opens over it.
- **Click targets:** see *Mouse* below; `a answer` and `?N` are two of them.

### Mouse

Everything the keys reach can be clicked, in both layouts. The renderer records
every target with the identity it was drawn for (orchestrator pane, thread,
agent) at the last paint; a card is registered before the labels inside it,
and the innermost target under the pointer wins.

| Click | Effect | Key |
|---|---|---|
| orchestrator card (narrow: its row) | select it, focus the column | `↑↓` |
| the selected orchestrator card, column focused | go to its pane | `g` |
| an orchestrator entry in the docked 3a panel | select it and switch to it at once (one click) | `↑↓` `enter` |
| thread card (narrow: the boxed card) | select it, focus the column | `→` `↑↓` |
| agent card, or a pod in a thread card | select it (the card expands) | `→` `↑↓` |
| the selected agent card, column focused | go to the orchestrator pane | `g` |
| `✓ n complete`, `n finished` | select and fold / unfold | `enter` |
| `+ New agent` | open the form | `n` |
| `all` / `running` / `needs me` in the header | that filter | `f` |
| `g open pane`, `t transcript`, `a answer` in an agent card | that action on that agent | `g` `t` `a` |
| `?N` on an orchestrator card | its questions in the inbox | `a` |
| empty space | nothing | |

"Clicked again" needs the card to be the cursor (selected *and* its column
focused): a selected card in a column without focus is only focused first, so
a click never acts on a card the operator was not already on.

- **Identity, re-checked.** A click acts on the identity drawn under it, never
  a screen position, and only if that identity is still shown (exists, passes
  the filter, is not folded away); otherwise nothing happens.
- **Arrival rule (herdr-inbox's rule for rows that arrive), per cell.** After
  every refresh, each screen cell whose click target changed (something
  appeared there, moved there, or another identity now sits where one was)
  ignores clicks for 1 s, including on the first paint. A click aimed before a
  refresh therefore cannot land on something the refresh slid under the
  pointer. It is per cell, not per identity, because one identity can be drawn
  in several places (an agent's pod in its thread card and its own card) and a
  card covers several rows: any one of those moving is enough. Only refreshes
  count: animation frames, hover and the wheel never change it (a frame keeps
  every card where it was). Comparing the cell maps costs well under a
  millisecond per refresh.
- **Selection clicks are harmless; action clicks are armed.** Selecting and
  filtering only move the cursor. Folding (`✓ n complete`, `n finished`) is
  armed like Enter, its key. Going to a pane, the New agent
  form, answering and the transcript path also wait out `ARM_SECONDS` after a
  refresh changed the selected card, exactly like their keys.
- **Wheel.** Over a column the wheel scrolls it by 3 rows, clamped to its
  content (a column that fits does not move). A wheel-scrolled column stops
  following the selection until the selection changes again (a key or a
  click); refreshes do not scroll it back.
- **Hover.** The border of the unselected card under the pointer brightens
  (narrow rows get a faint `▏`). A repaint happens only when the hovered card
  changes, not on every mouse move.
- Under the New agent form clicks reach only the form.

No database, an unreadable one or a missing table: no inbox marks, no `a`,
no `i`, no error.

## Threads

The hard part: a thread is a unit of work an orchestrator drives (a PR going
through author → review → fix → merge, a plan task, an investigation), and
Claude Code has no such concept. Sources, strongest first; an agent belongs to
the first that claims it.

1. **Explicit channel** (any flavour can emit it, see below).
2. **Native Workflow runs**: one thread per `runId`, named by the launch's
   `workflowName`; phases from the run summary's `phases`, else the journal's
   phase order, else `meta.workflowPhase`. Phase state: `done` when all its
   agents finished, `now` while one runs, `next` with none yet (`skipped`
   once the run ended), `failed` when its last agent failed.
3. **References** (`source: ref`): `#123`, `PR 123`, `PR #123`, `pull/123`,
   optionally with a repo word before it (`api #397`, `web #52`,
   `docs #688`), in the agent's description. Agents with the same key (repo
   word lowercased + number; a bare `#123` joins a keyed thread with the same
   number if there is exactly one) form one thread named after the key.
4. **Prompt eras** (`source: turn`, zoetrope's `prompt_for_ts`): remaining
   depth-1 agents are grouped by the operator prompt that was current when
   they were launched (the latest `origin.kind == "human"` prompt at or before
   the spawn). Name: the first agent's description, `+N` when there are more.

Nested agents (depth ≥ 2) join their root agent's thread.

**Heuristic phases** (sources 3–4): each launch and each resume of an agent is
a span `(role, start, end)`, its end the first notice after it. Spans sorted by start; consecutive spans with the
same role merge into one phase; phase names are the role, with a round number
for reviewer/fixer/author repeats (`review r1`, `fix r1`, `review r2`).
Heuristic threads never have `next` phases (the future is unknown).

Thread status: `active` while any agent runs, `failed` if its newest agent
failed, else `done` (`cancelled` if all were). Done threads outside the done
window (see Configuration) are dropped.

Flavour shown on the orchestrator card: the explicit channel's `flavour` if
any thread has one, else `workflow` if any thread is a workflow, else
`pr-review` if any is a `ref` thread, else `agents`.

Known weaknesses: an agent named "Fix flaky test" with no reference lands in
a turn thread even if it belongs to the PR the previous turn worked on; one
operator prompt that starts two unrelated pieces of work makes one thread; a
description that mentions two PRs joins the first. The explicit channel is the
fix for all three.

### Explicit channel (v1)

An append-only JSONL file per orchestrator session:
`${XDG_STATE_HOME:-~/.local/state}/herdr-panorama/events/<session_id>.jsonl`
(override `PANORAMA_EVENTS_DIR`). Each line is one object; later lines update
earlier ones field by field (`phases` and `agents` replace, they do not
append). Unknown fields are ignored; unknown `v` lines are skipped.

```json
{"v":1,"ts":"2026-10-14T09:43:00Z","thread":"pr-595",
 "name":"PR #595 soft-reuse ledger","flavour":"pr-review",
 "status":"active","worktree":"wt/ledger","pct":60,
 "phases":[{"name":"author","state":"done","start":"09:02","end":"09:20"},
           {"name":"review r1","state":"done","start":"09:24","end":"09:38","note":"1B 2H 3M"},
           {"name":"review r2","state":"now","start":"09:43"},
           {"name":"CI","state":"next"},{"name":"merge","state":"next"}],
 "agents":[{"match":"<agentId from the launch>","role":"author","phase":0},
           {"match":"review PR 595*","role":"reviewer","phase":3}]}
```

- `thread` (required): id, unique per session.
- `status`: `active` / `done` / `failed` / `cancelled`.
- `phases[].state`: `done` / `now` / `next` / `failed` / `skipped`; `start`,
  `end`: `HH:MM` or ISO-8601.
- `agents[].match`: an agentId, or a description (`*` suffix = prefix match).
  `role` overrides the derived role (the flavour's vocabulary, e.g.
  `task-reviewer`); `phase` pins the agent to a phase index.

Writers: `panorama emit --thread pr-595 --name … --phase "review r2=now"
--agent "review PR 595*=reviewer"` (session from `CLAUDE_CODE_SESSION_ID`,
which Claude Code exports to its Bash tool and subagents' Bash tools), or
any tool that appends the JSON line (a superpowers skill step, a review-loop
script, a coordinator's hook). It is deliberately a file, not a socket: no
server to run, no ordering problem between writers (O_APPEND lines under 4 KB
are atomic on Linux and macOS), readable after the fact. An MCP tool wrapper
(`declare_thread`) is a later step; the PoC documents the file and ships the
CLI.

## Claude Code plugin (opt-in)

The repo is also a Claude Code plugin (`.claude-plugin/plugin.json`,
`marketplace.json`, `hooks/hooks.json`). Nothing needs it: without it, the view
works from transcripts alone. Installing it adds exact start and stop records,
the `report_thread` tool and one line of session instructions.

### Hooks: never exit 2

For `SubagentStop`, exit code 2 "prevents the subagent from stopping" and its
stderr is fed back to the subagent. So the hooks do not go through uv or a
virtualenv at all. `hooks.py` is stdlib-only and runs directly:

```json
{"type": "command", "command": "sh",
 "args": ["-c", "python3 -I \"$0\" subagent-stop >/dev/null 2>&1; exit 0",
          "${CLAUDE_PLUGIN_ROOT}/src/herdr_panorama/hooks.py"], "timeout": 10}
```

- **Exit code:** the shell forces exit 0, and the script also ends with
  `os._exit(0)`. A missing or broken `python3`, an unknown kind, a bad
  payload or an unwritable directory all end as exit 0 with no output.
- **No shared environment:** there is no environment to share between plugin
  versions, so a plugin update during a running session cannot break a hook.
  Tested with a fake `uv` and `python3` that exit 2, and with 16 concurrent
  hooks from two plugin versions.
- **Speed:** about 20 ms.
- **MCP server:** it still runs through uv, with one environment per plugin
  version (`${CLAUDE_PLUGIN_DATA}/venv-<version>`).

### SubagentStart / SubagentStop

Payload fields, checked against the official hooks reference
(https://code.claude.com/docs/en/hooks):
- `session_id`, `agent_id` and `agent_type`; `SubagentStop` also has
  `stop_hook_active`. When it is true, Claude Code is already continuing
  because of a stop hook, and nothing is recorded.
- `SubagentStart` also fires when a subagent is resumed.
- The matcher `.+` skips Claude Code's internal agents (prompt suggestions,
  /btw), whose `agent_type` is empty.
- `agent-<id>` and `<id>` are both accepted.

Each event appends one line to
`${PANORAMA_HOOKS_DIR:-${XDG_STATE_HOME:-~/.local/state}/herdr-panorama/hooks}/<session_id>.jsonl`:

```json
{"v": 1, "event": "start", "agent": "a1b2…", "agent_type": "Explore", "ts": 1791983651.32,
 "pid": 4242, "pstart": "1234567", "pidns": "pid:[4026531836]"}
```

- **Process identity** is written only when the Claude process is recognised:
  the nearest ancestor that is a native install (`claude`, or a versioned
  binary under `…/claude/versions/`) or node running `@anthropic-ai/claude-code`.
- The identity has three parts: `pid`, `pstart` (the process start time:
  `/proc/<pid>/stat` field 22, or `ps -o lstart` on macOS, so a reused pid does
  not look alive) and `pidns` (the pid namespace).
- `SessionStart` appends a `session` record with the same identity, so a
  resume or restart shows up as a new process. It also prunes hook files older
  than 14 days.

How the scanner uses the records (on top of the status rules above):
- **A recorded stop is a confirmed ending** (`done`, not `done?`). A notice
  about the same stop, even a late one, says how it ended.
- **A recorded start reopens the agent**, like a resume.
- **`lost`** requires all of these:
  - a recorded start with no stop, notice or other ending after it;
  - its recognised Claude process, in our pid namespace, is gone;
  - and no later record comes from another process, meaning the session was
    not resumed.

  Anything uncertain falls back to `done?`. That includes an unrecognised
  process, another pid namespace, a resume, and endings without a stop record
  (a fork, a timed-out hook). This is the acceptance in issue #2.

### SessionStart instruction

Once per session (recorded in the hooks file) and inside herdr only
(`HERDR_PANE_ID` / `HERDR_ENV` set), the session gets one paragraph of
`additionalContext`, under 70 words:
- call `report_thread` when multi-step work starts and at each phase change;
- one call per change;
- skip it for single-step tasks.

### `report_thread` (MCP tool)

`report_thread(thread_id, title?, phase?, phases_done?, next?, status?, agents?)`
writes the explicit channel above, the same format `panorama emit` writes.

Reports merge:
- a phase gets `start` when it is first reported as current, and `end` when
  it is first reported as done;
- omitted lists keep what was reported before;
- `status` ends the thread.

Limits:
- at most 30 phases or agents per list;
- 200 characters per string;
- control and bidi/format characters are stripped;
- an agent pattern `abc*` must name at least 4 characters before the `*`.
  The channel reader enforces this too, so no declaration can claim every
  agent.

The events file is rewritten as one line per thread once it passes 64 KB,
under a lock that appends also take.

The session is resolved per call: the pane's current Claude session from
herdr (`HERDR_PANE_ID` → `pane get`, which follows `/clear`), else
`CLAUDE_CODE_SESSION_ID`.

## TUI: variant 2a

Rendered as a cell grid (one Textual widget that paints `Strip`s), copying the
design's characters, colours (`TH.dark`, pod colours `PB`) and coordinates.

- **Header**: `herdr panorama`, filters `all running needs me` (active one bold),
  counters `? N ! N ✗ N` on the right, clock `HH:MM`.
- **Wide (≥ 120 columns)**: three columns joined by solid links (`├─┬─┤`, see below):
  orchestrator cards (x 2, w 28): name, marks `✗ ! ?N`, `n active · n agents`,
  flavour tag; thread cards of the selected orchestrator (x 36, w 50): phase
  glyph + name, current phase + duration, pod row (finished pods fold to
  `+n done`), age tag; the selected thread unfolds into its steps with times;
  one `✓ n complete` card. Agent cards of the selected thread (x 93, w 44):
  pod, role, id, model, activity, age / calls tags; the selected agent expands
  with `now …`, questions, error, last message and actions `g open pane`,
  `t transcript`, `a answer`; finished agents fold into one
  `n finished · role ×n` card.
- **Narrow (< 120)**: indented list; only the selected orchestrator is open,
  its threads as cards, the selected thread expanded with steps and the
  selected agent's detail.
- **Legend** and **key hints** at the bottom, as in the design.

**Lines and animation (design turn 3).**
- **Lines:** links are solid box-drawing lines in the line colour. Where a link
  meets a card border the border becomes a tee: `├` leaving the source card,
  `┤` entering a target card, and `┬ ┼ ┴ ╰` on the trunk. Where the target's
  edge is animated it becomes `┨` (or `┠` leaving). Cells are merged bit by bit
  (up, down, left, right), so crossings and corners always join; the narrow
  list's tree uses the same lines.
- **Every card has an animated edge:** orchestrators, threads and agents. The
  design's `edge()`:
  - the left border takes the card's state colour, blinking when it needs you
    (failed, blocked, needs you): one second a blink, on for a frame, a sine fade
    off over the next, off for a frame, a fade back on (see Smooth animation);
  - while agents work, a 4-cell `━` bar in the running colour sweeps the
    bottom border, one cell per 250 ms frame;
  - the sweep is shifted by `seed × 7` cells, with seed 0 for orchestrators
    (they move together, as in the design), the index + 1 for threads, and
    the agent's own seed for agents;
  - idle, done and cancelled cards keep a plain border, as do panorama's
    extra `done?` and `lost`.
- **State:** a card's state is the design's `stOf` over its agents: attention
  > working > reviewing > idle, or done when everything is. Each orchestrator
  card shows it as a pod and a label, followed by
  `· <active threads>/<live agents>`.
- **Orchestrator-only extensions** (functional, consistent with the existing
  marks): herdr reporting the main session `working` counts as working,
  `blocked` as blocked, and inbox questions no agent filed as needs you.
  These are the main session's own state; an orchestrator often works without
  subagents.
- **Spinners** turn one step per frame.
- **Cost:** a frame never re-renders: it restyles the moving cells of the last render in place
  (see Smooth animation). No cell ever changes width.
- **Pausing:** frames are skipped only while nothing moves. Focus is ignored:
  herdr forwards focus-out to a pane whenever the operator focuses another one
  (measured on herdr 0.9.3), so app blur means "the operator is typing next
  door", which is where the docked panel lives, not "hidden". An unfocused or
  hidden view keeps animating at about 3 ms per frame, only while something
  moves.
- **Safety:** a frame is not a background change. It never touches the
  selection or the arming.

Columns scroll independently: each column keeps its selected card in view.
Agent cards are ordered by start time (the operator's decision; the design
orders them by urgency), so a status change never reorders cards under the
cursor. The pods in a thread card keep the design's urgency order; they are
not navigable.

### Wider cards on big screens (design turn 7, approved by the operator)

Reference: `docs/design/swarm-wide.js`, rendered by `render-7.mjs`; `tests/test_wide.py` compares
cell for cell at 160, 240 and 320 columns.

- **Width tiers** (orchestrators / threads / agents): below 220 columns 28 / 50 / 44 (the
  threads shrink to 36 below 160), as before; 220–299 columns 32 / 56 / 56; from 300 columns
  36 / 64 / 60, the cap. Columns still keep their width and align left. The space left over
  shows more depth columns: at 320 columns all five depth levels of the test thread fit.
- **The working sweep** along a card's bottom border takes at most 12 s per lap (48 frames),
  whatever the card's width. Up to a 44-column card it moves one cell a frame, as before; a
  wider card moves further each frame. The connector lines are not animated.
- **The model keeps its version:** `claude-opus-4-5-…` shows as `opus 4.5`. From 240 columns
  a card's type, id and model fit whole (`general-purpose 0c53fda · opus 4.5`); narrower cards
  cut the title with `…` before the state.

### Unseen (design turn 6, approved by the operator)

Reference: `docs/design/swarm-unseen.js`, rendered by `render-6.mjs`; `tests/test_unseen.py`
compares cell for cell (the whole 3a panel; every pulsed cell of 2a).

- **Events:** a new failure, a new question (a question id the agent did not have) or an agent
  finishing (done, or done? when inferred) marks that agent unseen. The most urgent mark wins:
  failure > question > finished. An answered question (no open question left) is not unseen
  any more; if the agent then finishes, that is a new event.
- **Seen:** a click on the card or its actions; Enter on it (on a `n finished` card: its folded
  agents); `g` / docking next to its orchestrator (all of that orchestrator's agents). Also herdr's
  focus *moving* onto the orchestrator's pane (focused at this refresh, not at the previous one)
  clears the marks that arose before it. Focus resting on a pane (the operator away, the cockpit's
  docked agent) is not looking, and the first refresh after start never clears by focus.
  Moving the cursor is not looking either.
- **Drawing:** an unseen agent card's whole border (corners, sides, the animated edge and its
  sweep) pulses in the event colour. A `n finished` card holding one does too. Thread,
  `✓ n complete` and orchestrator cards take the most urgent mark of their agents: the same
  pulse for a failure or a question, a quieter one for a finished agent. One pulse every 12
  frames (3 s), a sine wave in time, never fully off: full 35–100 % of the event colour over the line colour,
  quiet 15–45 %. The docked panel tints the entry's two lines (8–30 %, quiet 4–14 %) and
  pulses the docked list's thread glyph (and `✓ n done`). Unseen marks keep frames running.
- **State:** `unseen-<session>.json` in panorama's state dir, shared by every panorama process
  of the herdr session and kept across restarts. Remote agents are keyed `<machine id>:<agent id>`.
  The update runs on the collect worker and a seen mark is written by a worker thread: the
  file and its lock never block the UI thread. Each refresh compares the swarm with the
  file under a short lock (0.2 s; busy: this tick keeps its marks), so an event is reported
  once and a seen mark clears everywhere. The first run only records; later, an agent that
  appears already finished counts only if it ended after the last save. Agents not seen for
  a week are forgotten. `highlight_unseen = false` turns it all off.
- The narrow layout (< 120 columns) and the docked panel's spawner rows have no pulse.

### Smooth animation (design: swarm-smooth.js)

The operator's report: colour animations moved in visible steps and looked laggy, because every
colour changed only on the 250 ms frame. Reference: `docs/design/swarm-smooth.js` (used by
`swarm-min.js`, `swarm-dock.js`, `swarm-unseen.js`, `swarm-deep.js`, `swarm-tiles.js`), rendered between
whole frames by `render-smooth.mjs` (2a with unseen marks, the 9e panel with and without);
`tests/test_smooth.py` compares those frames cell for cell. 9e's needs-you tiles ease between their
colours and `TILE_OFF` the same way, and an unseen tile's pulse runs over that blink.

- **Time, not steps.** `View.frame` is the animation time in 250 ms frames and may be
  fractional. Whole frames move spinners and sweeps, one step a frame, as before. Colours follow
  the time itself: the unseen pulse is a sine of it, and the attention blink eases on and off
  with a sine ramp (on 1 frame, fade 1, off 1, fade 1). At whole frames every colour is exactly
  the old stepped one, so the golden frames of turns 3, 5 and 6 are unchanged.
- **The colour tick:** `animation_fps` (default 20, clamped to 2..30, read when the view starts;
  the operator's choice). It runs only while something on screen moves, like frames. Between
  frames it runs only while a pulse or a blink is visible. Each tick costs no render: the last
  render's moving cells hold `render.Anim` values (a str that knows how to recompute itself:
  colours, spinner glyphs, sweep cells). `restyle` recomputes them in place and only the cells
  that changed are repainted. At a lower fps a tick can span several frames, so spinners and
  sweeps keep their speed, only steppier.
- **Renders** happen only on a refresh, a key, the mouse or a resize, and then only the spans that
  differ from the screen on display are repainted. A frame where the docked panel's hold ran out
  repaints in full (the panel may turn back into the graph).
- **Textual crop.** Textual 8 sends a partly updated line from the widget's left edge, so a few
  cells in the middle of the one full-width grid re-sent most of the screen. `tui.py` replaces
  `ChopsUpdate.render_segments` with one that crops both sides (tested).
- **Measured** (isolated herdr 0.9.3, the design dataset at 160×45 with five unseen pulses and
  blinking edges; CPU of panorama + herdr server + herdr client): before, 4 fps full renders,
  4.7 + 0.9 + 1.3 = 6.9 %. A naive full render at 20 fps costs 22.7 + 4.0 + 5.5 = 32 %. With the
  colour tick at 20 fps: 8.1 + 1.9 + 5.1 = 15 %; at 10 fps 7.1 %; 15 fps 11.5 %; 30 fps 22 %.
  With blinks but no pulses, 20 fps costs 2.9 + 0.8 + 2.9 = 6.6 %. Bytes from panorama per
  tick: 35 KB per full frame before, about 1.3 KB per tick now. herdr diffs pane output, so its
  client only redraws what changed; its cost grows with the fps, not with the screen.

### Waiting and stale (design turn 8, approved by the operator)

Reference: `docs/design/swarm-states.js`, rendered by `render-8.mjs`; `tests/test_states.py`
compares cell for cell (the whole docked panel, 9e since turn 9; in 2a the new orchestrator cards, the state lines
of the earlier ones, the background card and the waiting agent's card). The operator's
answers: the 3a questions label is now `needs you` (Q109), the header chip `refresh failed`
(Q110), and a long wait stays `waiting` with its age (Q111).

- **Waiting** (`bg`, ⠁, lavender `#b39ddb`): herdr says the session is idle (or an agent's
  turn ended), but background work it started is still running, so it will wake up by
  itself. One braille dot circles the cell, a step per second (`⠁⠈⠐⠠⢀⡀⠄⠂`; the design's first
  draft had the clocks `◴◵◶◷`, but ghostty takes those from a wider fallback font, the problem
  that replaced `◐◓◑◒` with braille); the left edge is steady (no sweep:
  nothing thinks; no blink: nothing needs you).
- **Stale** (◌, grey `#646a73`): an orchestrator that would be idle or all done, with nothing
  in flight, whose last activity is older than `stale_after` (default `"12h"`). Last activity
  is the newest timestamp in its session's transcript or any of its agents' (a prompt, a
  reply, a tool call, a notice). Name and label go faint: `stale 15h` (hours up to 48, then
  days). It keeps its place in tab order.
- **Order:** failed > blocked > needs you > working > reviewing > waiting > stale | idle |
  all done. A waiting orchestrator older than `stale_after` stays waiting and says how long:
  `waiting 15h` (the oldest task's age).
- **2a:** the orchestrator card reads `waiting · N bg`. Its background tasks are a
  `background` card above its threads (selectable; Enter stays: nothing is behind it), one
  row per task: clock, kind (`Bash`, `Monitor`), description, age; the bottom tag says
  `turn ended Nm ago` (since the last activity). A waiting agent's expanded card lists its
  tasks under `now  turn ended · N task(s) in background`, and its label is `waiting 3m`
  (since its oldest task).
- **Docked panel (9e tiles):** the state tile and the label `waiting` (or `stale 15h`), the
  second line appends ` · N bg`, and the docked entry lists its background tasks first, a
  lavender tile each (`bash 14m` on the right). An entry without threads starts its counts
  where the tiles would be. In 9e too, questions read `needs you` (Q109).
- **Animation:** the clock is an `Anim` on the restyle path (`render.clock`), stepping at whole
  frames like the spinner; `tests/test_smooth.py` covers the 2a and 9e waiting layouts.
- **Filters and counts:** `running` keeps waiting orchestrators and drops stale ones; `needs
  me` shows neither. The header's `? ! ✗` are unchanged. The legend gains `⠁ waiting` and
  `◌ stale` (a narrow legend drops whole entries that do not fit: stale, waiting, lost, done?).

**Detecting background work (`scan/session.py`).** Records, all captured in
`tests/fixtures/templates.json`:

| Record | Meaning |
|---|---|
| `toolUseResult.backgroundTaskId` on a Bash result | a Bash in the background (`run_in_background`, or moved there by its timeout: `timedOutAfterMs`) |
| `toolUseResult` `taskId` + `timeoutMs` (+ `persistent`) | a Monitor watch; it expires after `timeoutMs` unless persistent |
| `<task-notification>` with a `<status>` naming the task | it ended. **Every** `<task-id>` of a notice counts: a resumed session gets one notice listing all the tasks its previous process left, plus Claude Code's own `__orphan_summary__` marker id (skipped) |
| a notice whose `<event>` starts with `[Monitor expired` | the Monitor ended |
| a TaskStop / KillShell result, not `is_error` (`task_id` + `task_type`) | stopped |

A task belongs to the file that launched it (main, or an agent; a fork's copy of its parent's
history never claims it). It is open until one of the endings above, or a Monitor's timeout
(+60 s). Claude Code can also end a task without any record (a stop from its UI, the process
exiting), so an open task is only *waiting* while:

- **its process lives (local machines, Linux):** a background Bash runs as a shell under the
  Claude process, writing `…/tasks/<task id>.output`; while it runs, a descendant of the
  pane's Claude process (`herdr pane process-info`, cached per pane, re-checked by start
  time) holds that file open (`bgproc.py`, `/proc/<pid>/task/*/children` and
  `/proc/<pid>/fd`). A shell left by an earlier Claude process (the session was resumed) is
  not a descendant: nobody will be told it finished. The tree is read once per collect and
  only when a task is open.
- **otherwise at most `BG_CAP` = 24 h after it started:** another machine's mirrored
  transcripts (`machines.py`: transcript only), no `/proc` (macOS), a Monitor (its process is
  not visible this way; its timeout usually ends it first), or a pane whose Claude process
  herdr cannot name.

An agent is waiting when, without terminal evidence for it (a notice, a completed
acknowledgement, a journal result: the **completion guard**), it is not writing (120 s), has
no call in flight and still has a waiting task. Its own notice only comes once its background
children are gone, so a finished agent's leftover loops never make it waiting (seen on the
operator's machine: poll loops of agents done days ago, still alive). An agent waiting on a
waiting child counts as running, as for a running child.

**Validation** (read-only, on the operator's 30 days of transcripts, transcript rule alone):
of 1,408 idle gaps of main sessions (an `end_turn`, then over a minute of silence) the rule
called waiting, 1,361 had that task's ending notice arrive later (96.7 %), 6 were tasks still
running, and 41 belonged to one task stopped without a record (the process check removes
it). One idle gap was woken by a task the rule did not know (launched under an earlier
session id, before a resume). Live: of 7 idle orchestrators, one was waiting (a poll loop,
a child of its Claude process for 15 h, checked by hand); the other six were idle for 13–39 h
(stale). The replay oracle's counts for agents are in *Accuracy*.

### Subagents to any depth (design turn 5, direction 5d)

Agents spawn subagents, which spawn theirs. The collector already puts a nested
agent in its top agent's thread with its `parent`; the view draws that tree
instead of flattening it. Reference: `docs/design/swarm-deep.js` (5d), rendered
by `render-5d.mjs`; the golden test `tests/test_deep.py` compares cell for cell.

- **One column per depth:** orchestrators | threads | depth 1 | depth 2 | … as
  deep as the selected thread's tree. A depth column holds every shown agent of
  that depth in the thread. Each card sits level with the agent that spawned it
  (or below the cards already in its column), and 2a's links join it to that
  agent. An agent with children starts below the previous group's last child,
  so two groups' lines never share rows. Finished children fold into one
  `n finished` card per parent; Enter unfolds it, as in the depth-1 column. A
  finished agent still shows while a live or failed agent below it does.
- **Columns keep their width and align left** (agents 44, the last one may
  shrink to 36 to fit). The space left on the right stays empty: the operator
  approved this in turn 5, because a stretched card's sweep runs too long. This
  applies to the depth-1 column of a flat thread too, which used to stretch to 65.
- **Sideways scrolling:** the view shows the focused column, the column before
  it (its parents) and, when it fits, the column after it (its children). Then
  it adds more columns while they fit, right first, then left. The line above
  the columns names what is off-screen: on the left (`‹ …`) the selected path
  through the hidden columns, on the right `depth a–b  n columns right ›`. A card
  whose children's column is off-screen shows `› n`. shift+wheel or a horizontal
  wheel scrolls without moving the focus; the next focus or selection change
  scrolls back to it. `shift+wheel scroll` joins the key hints while something
  is off-screen.
- **Keys:** `↑↓` move within a depth column, across parents. `→` goes into the
  selected agent's children: the child last selected there, else the first one.
  `←` goes up to the agent that spawned it. From the thread, `→` steps onto the
  depth-1 agent on the selected path.
- **Thread cards** show pods for the depth-1 agents only, then `+N nested`.
- **Narrow layouts (< 120 columns) have no depth view.** The design specifies 5d at
  160 columns and the 46-column panel only. The narrow list keeps 2a's thread card,
  with one pod row for all of the thread's agents (nested ones included), plus the
  selected agent's details. `→`/`←` still step through the depth columns, but the
  siblings being walked are not drawn; only the selected agent's details show where
  the cursor is.
- **Scrolling never moves a click target silently.** A wheel or sideways scroll
  slides other cards under a still pointer, and trackpad momentum keeps scrolling
  while the operator clicks. Every cell whose target a scroll changed ignores clicks
  for 0.3 s. This is the refresh rule with a shorter window, so a deliberate
  scroll-then-click still works.
- **The docked panel (46 columns)** keeps each thread row with its depth-1
  agents' glyphs. Under the row is an indented tree of only the agents that
  spawned others, each followed by its own children's glyphs (`lead l1  ⠋⠹ +2✓`).
  Leaf agents stay glyphs.
- **Data:** the collector walks spawn chains to any depth (the old walk stopped
  after 8 hops), places parents before children, and keeps a finished ancestor
  of any shown agent even when it is outside the done window. A parent loop or a
  parent outside the thread makes the agent a depth-1 one; nothing is hidden.

Keys: `↑↓`/`jk` move in the focused column, `→`/`l`/`enter` open (focus the
next column), `←`/`h` back, `g` go to pane (focus the orchestrator's pane and
close the popup), `t` transcript (show the agent's transcript path; opening a
viewer is a later step), `a` answer (opens herdr-inbox when the selection has
an inbox question; otherwise `g`), `i` inbox, `f` cycle filter, `?` help,
`q`/`esc` close.

## herdr plugin surface

`herdr-plugin.toml`, id `leavemyyard.herdr-panorama`, `min_herdr_version =
"0.9.0"`:

- pane `panorama`: `placement = "popup"`, `width = "100%"`, `height = "100%"`
  (the design's full-screen overlay; popups are session-modal and leave the
  layout untouched);
- pane `new-agent`: `placement = "popup"`, 84×26 cells, the `+ New agent` form
  alone for panes too narrow for it;
- action `open` (popup) and action `open-split` (`herdr plugin pane open
  --placement split --direction right`, the design's 70-column side split);
- `[[build]]` `uv sync --frozen --no-dev`.

Key: `prefix+space` (the operator's choice; herdr accepts it without diagnostics). The README
gives the snippet; the plugin never edits the herdr config.

```toml
[[keys.command]]
key = "prefix+space"
type = "plugin_action"
command = "leavemyyard.herdr-panorama.open"
description = "swarm view"
```

## Home tab and new agents

herdr cannot replace its own main UI, but a plugin pane can have a tab of its
own; panorama uses that as the operator's home surface. The operator asked
for both features: the home tab, and a "New agent" button below the chats. In
the TUI they follow design 2a's visual language: the `+ New agent` row is a
card like the orchestrator cards, with a `+` in the key colour and a dim
label, and the form uses the same palette. `◆ panorama` is the herdr tab label.

- **Single instance.**
  - The home pane is opened with `herdr plugin pane open --placement tab`
    (environment `PANORAMA_HOME=1`) and renamed `◆ panorama`.
  - It is moved to the first position with the socket API's `tab.move`
    (`{"tab_id", "insert_index": 0}`); the CLI has no `tab move`.
  - It is found by content: the pane labelled `◆ panorama` that runs no agent and
    whose foreground process is `panorama tui`, wherever it is (see *Cockpit*). Its id is remembered per herdr session in
    `$HERDR_PLUGIN_STATE_DIR/home-<session>.json` only as a tie-breaker; duplicates
    are closed.
  - An older home pane (the only agent-less pane of a tab labelled
    `◆ panorama`) is adopted and labelled.
  - Opening runs under a file lock and re-checks after taking it, so the
    startup hook and the key cannot open two (tested with 6 concurrent
    openers).
  - A failed or timed-out snapshot means "unknown", never "missing": nothing
    is opened and the record is left alone. The key falls back to the popup,
    and the next start or key press tries again.
- **Opening and returning.** The `[[startup]]` hook runs `panorama home
  --startup`: it opens the tab once and does not take focus. The `open` action
  (`prefix+space`) focuses the home tab, or opens it if it was closed, so the
  same key brings you back home from anywhere. `open-popup` always shows the
  popup. With `home_tab = false` in the plugin config, `open` shows the popup
  as before.
- **In the home tab** the view never closes itself:
  - Enter on an orchestrator, and `g` on any card, focus that agent's pane;
  - `a`/`i` open the inbox popup over it;
  - Esc does nothing, and `q` quits.
- **No `◆ panorama` tab without panorama** (issue #9). If the operator splits the
  home tab and starts an agent there, the tab must not keep panorama's label once
  panorama is gone: it would read as a stale panorama tab, and closing it would
  kill the agent.
  - On `q`, before exiting, a home tab that still holds other panes is renamed to
    the remaining agent's herdr name, else its kind (`claude`), else `tab`. It is
    the same rename the cockpit does before docking out of a shared home tab.
  - A crash skips that, so every `ensure` (the key and the startup hook) sweeps
    first: every `◆ panorama` tab with no panorama in it is renamed the same way.
  - A tab keeps the label while any agent-less pane in it is verified panorama, or
    cannot be checked (process-info fails, or no foreground process yet, as
    mid-start or mid-restore).
- **`+ New agent`.** It is a row under the orchestrator cards with the stable
  id `new:`, always last, and the key `n` opens it too. A refresh never moves
  it or changes what Enter does on it, and Enter on it is armed like every
  other action. The form asks for:
  - a name (required, slugified to herdr's agent-name rule
    `[a-z][a-z0-9_-]{0,31}`, also the tab label, and refused if
    `herdr agent get <name>` already resolves an agent);
  - a working directory (default: the selected orchestrator's cwd, read once
    when the form opens; it must exist);
  - an optional first prompt.

  The form is 76 columns wide. In a pane narrower than 80 columns (the docked
  panel's 46) it opens as its own herdr popup over everything instead, the
  plugin's `new-agent` entrypoint (`panorama new-agent`, 84×26 cells), with the
  directory passed in `PANORAMA_NEW_AGENT_CWD`. It is the same form, with the
  same arming, validation and agent creation, and the popup closes with it.
  This fixes the operator's report that the form was cut in the side panel. If
  the popup can't open, the form opens in the pane as before. The full view
  keeps the form in place, where it fits.

  While the form is open, the view's own keys are inert behind it. Submit is
  armed 0.4 s after the form appears, and Enter on the last field or ctrl+s
  submits. The field values are read on the UI thread at submit and the
  inputs are disabled until the result, so typing after submit changes
  nothing. A worker then runs:
  1. `herdr tab create --label <name> --cwd <dir> --no-focus`;
  2. the new tab's pane, from `pane list`;
  3. `herdr agent start <name> --kind claude --pane <pane> --timeout 60000`,
     which waits for readiness. If anything fails up to here, the tab is
     closed again and the error is shown in the form.
  4. if a prompt was given, `herdr agent prompt <pane> "<prompt>"` once:
     sent to the pane id just created, never to a name. herdr reads the word
     after the target as the text whatever it looks like, so a prompt such as
     `--wait` stays text (a `--` there would itself become the text);
  5. `herdr tab focus <tab>`.

  This is the only place panorama types into a pane, and only on the
  operator's submit. The prompt passes `herdr.clean`: line breaks become
  spaces, and control and escape characters are dropped.

## Cockpit (experimental)

The operator hides herdr's own sidebar and wants panorama as an agent list on the left of
the agent they are working with. `cockpit = true` in the plugin config turns that on;
`false` (the default while experimental) keeps the plain home tab.

**The only pane that ever moves is panorama's own.** Selecting an agent moves panorama's
pane into *that agent's* tab, docked on the agent's left at a fixed width, and focuses the
agent. Agents never leave their tabs. (The first version moved the agent into panorama's
tab; review showed that closing "panorama's" tab would then kill a live agent, so the
design was inverted.)

### What herdr 0.9.3 does (measured)

All measured on throwaway shell panes and a panorama plugin pane in throwaway workspaces I
created and closed; no pane of the operator's was touched.

- **`pane move <pane> --tab <tab> --split right [--target-pane P] --no-focus`** moves a live
  pane, plugin panes included (panorama's TUI kept running, same `terminal_id`).
  - Within a workspace the pane id is unchanged; across workspaces it changes
    (`w7:p2` → `w8:p1`). Follow `.result.move_result.pane.pane_id`.
  - The pane's label (`pane rename`) moves with it.
  - A tab whose last pane leaves is closed (`closed_tab_id`), an emptied workspace too.
  - With `--no-focus` the operator's focused workspace and tab stay as they were.
- **`pane move <pane> --new-tab --workspace W --label L --no-focus`** puts it in a new tab
  (appended); the socket API's `tab.move {tab_id, insert_index}` restores a position.
- **`pane swap --source-pane A --target-pane B`** swaps two panes *of one tab*; across tabs
  it returns `changed: false, reason: "cross_tab"`. It has no `--no-focus`: it focuses the
  source pane, switching the operator's workspace and tab to it.
- **`layout.set_split_ratio {tab_id, path, ratio}`** (socket API) sets one split exactly;
  `path` walks from the root, `false` = first child, `true` = second, and `ratio` is the
  first child's share (46/296 gave 46 columns). `layout.export {tab_id}` gives the tree;
  `pane.layout {pane_id}` gives every pane's rect.
- **`layout.apply` recreates panes.** Applying a tree that referenced the tab's existing
  panes produced a new tab with new shells and closed the old panes, processes included.
  Panorama never uses it.
- **Docking into a tab with several panes** splits the agent's own cell, not the whole
  tab: panorama sits left of that agent pane at the agent pane's height.
- **herdr-inbox** delivers to the stored pane id after checking that pane's session; agents
  never move now, so delivery and the inbox's `❓N` tab marks are untouched.

### Design

- **Single instance, found by content.** panorama's home pane carries the pane label
  `◆ panorama`, runs no agent, and — the proof — read-only `pane process-info --pane <id>`
  shows panorama's view in its foreground: an argv with `panorama` (any path: the `uv run`
  wrapper and the venv script both appear) followed by `tui`. Labels alone are only
  look-alikes: a shell left after panorama quit, a pane whose agent just exited, another
  plugin's pane alone in a `◆` tab, or a label herdr restored without its process. A
  look-alike is never adopted, renamed, moved or closed. An older home pane (the only
  agent-less pane of a tab labelled `◆ panorama`) is adopted and labelled only when it
  passes the same check. If a look-alike cannot be checked (process-info fails, or the
  pane has no foreground process yet, as mid-exec or mid-restore), nothing is adopted and
  nothing new is opened; the key falls back to the popup.
- **One panorama is used, none is closed.** The calling instance's own pane is preferred
  (Enter in instance A moves A), else the remembered one, else the first. A second
  verified panorama is left alone: neither adopted nor closed. `plugin pane close` on a
  tab-placement plugin pane that has since moved into another tab is not measured (it
  might close the tab it was opened in), so panorama never closes panes.
- The record is only a hint, and it is per herdr session
  (`home-<hash of the socket path>.json`), so two `herdr --session` servers never share
  one.
- **Docking** (Enter on an orchestrator, `g`, or a click on the selected card), in a
  worker, one at a time, under the home lock:
  1. A fresh snapshot; panorama's pane by content; the target must run an agent. Already
     in the target's tab: only the width is fitted. The cockpit setting is read live at
     each Enter, so `cockpit = false` stops docking at once.
  2. **Nothing is left behind under panorama's label.** If panorama is leaving a tab
     labelled `◆ panorama` that still holds other panes (the operator split the home tab
     and started Claude there), that tab is first renamed to the remaining agent's name
     (else its kind, such as `claude`, else `tab`). If the rename fails, panorama does not move.
  3. `pane move <panorama> --tab <agent's tab> --split right --target-pane <agent>
     --no-focus`. A failure leaves panorama where it was.
  4. `pane swap` puts panorama on the left (a failed swap leaves it on the right, logged).
  5. The split holding panorama is set to `cockpit_width` columns.
  6. `agent focus <agent>`. This also runs when panorama could not move: the agent never
     moved, so going there is always safe.
- **Back to the list.** `prefix+space` finds panorama wherever it is docked, focuses its
  tab and then its pane (`pane.focus`).
- **Losing panorama is harmless.** Closing an agent's tab closes that agent (what the
  operator expects from closing its tab) and panorama's pane with it; the next
  `prefix+space` or start opens a new home tab.
- **Cockpit off.** `ensure` moves panorama back to a tab of its own, labelled `◆ panorama`
  and first (`--new-tab --workspace <its workspace>`, then `tab.move 0`), only when it is
  docked by an agent: in a tab that is not a home tab. A home tab the operator split is
  left exactly as it is, as on `main`.
- **Width.** Re-fitted after each dock and when the shared split's width changes (a
  terminal resize). The resize fit never runs while a dock is under way, takes the same
  lock as docking, and sizes only a verified panorama pane (never the record's). The last
  width is kept in memory, so a split dragged by hand stays until the next resize.
- **Drawable width, not rect width.** herdr draws a frame around panes, so a pane's
  terminal is narrower than its layout rect. On the operator's session the docked pane's
  rect was 46 columns and panorama got 44, which clipped `working`, `done`, `idle` and the
  footer. The frame varies: measured with `stty size` inside throwaway panes, it was 1 to 3
  columns depending on the pane's side and the session.
  - `cockpit_width` is therefore the *drawable* width. The split is set to
    `cockpit_width + frame`.
  - The view measures the frame itself: its pane's rect width minus the width it is drawn
    at, only while it shares a tab (docked).
  - A new measurement re-fits even at the same tab width. Live, the first fit drew 47 or 46
    columns, and after measuring, both sides drew exactly 46.
  - Before the first measurement the fallback is 2, the operator's.
- **Any width renders.** The 3a panel lays out to the width it is actually drawn at.
  - Right-aligned fields (state, `?N`, the thread agents, `n`) end at the real right edge.
  - A narrow pane cuts names, never the state label.
  - The footer drops whole hints (`n new`, then `a answer`, ...) rather than cutting one.
  - At the design's 46 columns it is still cell for cell the design (`tests/test_dock_panel.py`).
  - The 2a layouts already draw to the size they get; nothing else assumed rect equals
    drawable width.

### Follow (`cockpit_follow`, default true)

The panel used to exist only in the tab it docked into. If the operator switched tabs
another way (the tab bar, `prefix+1..9`, a notification, `herdr agent focus`), the new tab
had no panel. Now panorama listens to herdr's focus events and re-docks its own pane
wherever the operator lands on an orchestrator. Because this moves a pane without a key
press, the rules are strict:

- **Listening (read-only).** The home view keeps one `events.subscribe` connection for
  `tab.focused` and `pane.focused`. Events arrive as
  `{"event": "tab_focused", "data": {"type": "tab_focused", "tab_id", "workspace_id"}}`;
  this was captured from the live server with throwaway workspaces. A lost connection
  reconnects with backoff.
- **Once per switch.** Each event restarts a 150 ms timer, so a burst (a tab switch sends
  both events; `prefix+1`, `prefix+2` in a row) is acted on once, for where the operator
  ended up. While a follow runs, new events only queue one more look after it, so two never
  run at once.
- **Never while:**
  - an explicit dock is running;
  - the New agent form is open;
  - the cockpit or `cockpit_follow` is off (both re-read on each event).
- **The decision** (`cockpit.follow`) runs on a fresh snapshot, under the same lock as
  docking, with panorama's pane verified by content. It moves only when herdr's focused
  pane is a **Claude** agent (`agent == "claude"`) in **panorama's workspace**, in a tab
  panorama is not already in. It never moves for:
  - shells, the inbox popup or other plugins' panes;
  - other agent kinds;
  - other workspaces;
  - panorama itself;
  - nothing focused.
- **The move** is the same path as an explicit dock (`_move_next_to`):
  1. the rename before leaving a shared home tab;
  2. `pane move --no-focus`;
  3. the split ratio.

  A failed step leaves everything as it is.
- **Tabs** (`cockpit_follow_tabs`): `"any"`, the default and the operator's decision, also
  follows into tabs the operator split, where panorama then takes about 46 columns of that
  layout. `"single"` follows only into tabs whose one pane is the orchestrator. An explicit
  Enter docks anywhere with either setting.
- **Never while zoomed.** While panorama is zoomed (`z`, `pane.layout` reports `zoomed`),
  the operator is looking at the full graph, so panorama stays put.
- **Focus.**
  - `move --no-focus`, the ratio and the layout reads never change focus. This was verified
    live on a throwaway workspace, comparing the operator's workspace, tab and pane before
    and after.
  - Only `pane swap` (which puts panorama on the left) touches focus. It has no
    `--no-focus` and focuses its *source* pane, so the swap's source is always the agent,
    the pane the operator is already on. A swap in a tab the operator has left pulls them
    back there; the reviewer measured this on an isolated server. Explicit docks use the
    same swap direction, so focus lands on the agent directly.
  - **Right before anything changes**, the follow path takes one more snapshot. If the
    operator is no longer on the agent (say they flipped to its tab and straight back to
    panorama during the decision's few herdr calls), it stops: no rename, no move.
  - **The check-to-swap window is closed with the event stream panorama already holds.**
    Every focus event is stamped on arrival in a `FocusLog`.
    1. Just before the swap, a fresh snapshot must still show the agent focused.
    2. After the swap, panorama waits up to 0.3 s for the agent's `pane_focused`. If the
       operator had switched away, the swap pulled them back and herdr reports exactly
       that, after their own switch, since events are in order. In the normal case the
       agent was already focused and the swap sends no event at all (measured), so this is
       a bounded settle wait for late events.
    3. If a focus event since the check points elsewhere, panorama sends focus there:
       `pane.focus` for a pane, or `tab focus` for a tab-only event, so herdr restores that
       tab's active pane.
    4. It re-reads the log once after the refocus, in case the operator kept going.
  - **Only a live stream counts.** `events()` marks `_connected` when herdr acknowledges the
    subscription and `_disconnected` when the stream is lost; `FocusLog.live` follows them.
    While it is not live (not connected yet, or backing off), the follow path does not
    swap: panorama stays on the agent's right, and the next explicit dock puts it on the
    left.
- **One follower.** Only the home instance (the remembered pane) follows. A second panorama
  pane during startup lag ignores focus events instead of docking beside the first.
  - `layout.apply` is never a swap substitute: it recreates panes and kills their
    processes.
- **One client.** herdr's focus events and the snapshot's `focused_pane_id` do not say which
  attached client switched; they are the server's view. With two clients attached to one
  session (say a laptop and `herdr --remote`), a tab switch in one moves panorama for both.
  Turn `cockpit_follow` off in that setup.
- **The event stream's backoff** grows from 0.5 s to a 10 s cap on failed connects. It resets
  only after an ack or an event actually arrives, so a server that accepts and closes cannot
  make panorama reconnect every 0.5 s.
- **Zoom.** `z` in the home view toggles `herdr pane zoom` on panorama's own pane: full
  screen, which gives the 2a graph, and back to the docked panel. It only runs on the
  operator's key while they are in panorama, so it never moves focus elsewhere. Its hint is
  `z graph` on the panel's footer rule (the design's footer has no room left) and `z panel`
  on the graph's key line.

### Latency (switching chats)

The operator reported that switching chats from the docked panel lagged: for a few seconds
it showed the full 2a graph and/or kept the previous agent highlighted. Measured on
throwaway panes (`PANORAMA_TRACE=<file>` logs every herdr call and each dock or follow step
with its duration):

| Step | Time |
|---|---|
| a whole explicit dock (snapshot, process-info, move, swap, layout, ratio) | 25–50 ms |
| the same without the swap | median 13 ms |
| the follow decision | median 3 ms |
| one herdr call | 2–14 ms each; the swap ~10 ms |
| a collect on the operator's live data | 6 ms (1.9 s on the first, cold one) |

So herdr calls were never the lag, and switching to the socket API would save milliseconds;
they stay CLI calls. The lag came from the view:

- **The graph flash.** `pane move` first gives panorama half the agent's tab, then the
  ratio shrinks it. In between the view saw a wide pane and drew the 2a graph.
  - Now the docked panel is kept while a dock runs, while a move of panorama's pane is
    under way (`cockpit.moving_until`, which covers the follow path too), and for 1 s
    after a successful dock or follow.
  - The layout is re-decided after that, so a deliberate zoom (`z`) still shows the graph.
- **The stale highlight.** Which agent is docked came only from the next collect (up to
  1.5 s).
  - Now a successful dock or follow shows it at once: the worker calls `on_docked` right
    after the move, the view runs `show_docked`, and a collect starts immediately. The
    agent comes from that call, not from a process-wide variable.
  - A collect that began *before* the dock keeps the new value. One that began after it
    reconciles with herdr.
- **The follow settle wait stays at 0.3 s, but nobody waits for it.** On an isolated
  server, herdr delivered focus events ~50 ms (median) and up to ~100 ms after the action
  (a ~100 ms flush tick), so the guard needs that margin. The view is told about the new
  dock right after panorama's move succeeds (`on_docked`), before the swap and the settle
  wait. The highlight therefore moves at once while the guard keeps its 3× margin.

Every safety guard is unchanged: verification by content, the focus re-checks before the
move and the swap, the lock, the arming and identity rules.

**No waiting across the lock.** The early report runs inside the dock lock, so it must never
wait for the UI thread. It is a message posted thread-safely (`post_message`), not
`call_from_thread`. The home lock itself never waits forever:
- `ensure`, dock and follow give up after 10 s (`home.LockBusy`); the view's resize fit after 2 s.
- Quit, which runs on the UI thread, waits at most 0.5 s for its relabel and otherwise skips
  it; the sweep in the next `ensure` covers the skipped relabel.

A stuck panorama therefore cannot hang the key, the startup hook or itself. Workers hand
their results to the view only after releasing the lock.

### The docked panel (designs 3a and 9e)

When panorama is docked next to an agent (it shares a tab with an orchestrator) and is
narrower than the 2a wide layout, it draws the operator's pick from design turn 3:
**3a "Compact list"** (a port of `vA` in `docs/design/swarm-dock.js`; it replaced 3b "Cards
with a pod row", which the operator tried live first), with the coloured tiles of turn 9's
**9e "tiles, list replaces row"** (`docs/design/swarm-tiles.js`, rendered by
`docs/design/render-9.mjs`; the operator's pick of 9a–9e). A tile is 2a's pod: ` g ` in
the state's tile colours, bold; a needs-you tile blinks like the bare glyphs do, to a grey
tile in its off phase.

- **Header.** `SWARM  N orchestrators`, with `?N !N ✗N` on the right.
- **One entry per orchestrator, two lines.**
  - The first line: tab number, the state in a tile, name (dim when all done), then `?N`
    and the state label (`failed`, `blocked`, `working`, `needs you`, `waiting`, `stale`, `idle` or `done`;
    turn 8 renamed the questions label from `waiting` to `needs you`).
  - The second line: one tile per active thread (its most urgent agent's state), a cell
    apart; the done threads fold into one `✓n` tile; then `N active · N agents`. When the
    tiles do not all fit, the first ones then `+n`, so the counts stay whole.
- **The docked entry** is the orchestrator in panorama's own tab. It has a gold `▌` bar on
  the left and the selected background, and it lists its threads underneath as a tree: a
  tile, the name (up to 22 cells), and on the right the thread's live agents as bare glyphs
  (most urgent first, up to 4, then ` +N✓`). Done threads fold into a `✓` tile and `N done`.
  Its second line keeps only the counts, since the list shows the same tiles.
- **Unseen** (turn 6). The entry's two lines pulse as a tint, as in turn 6. In the docked
  list an unseen thread's tile, and the done fold's tile, pulses its background toward the
  event colour (red, amber, green), every 3 s and never fully off (the operator, Q115: "tile
  background pulses").
- **Below the entries:** the `+ New agent` row. **Footer:** `↑↓ move  ret open  a answer
  n new  pfx+spc back`.
- **State.** Computed like the design's `oState`, with the same extensions as 2a: herdr
  reporting the main session working counts as working, blocked counts as blocked, and the
  session's open inbox questions are its `?N`. The main session's own thread and agent are
  the entry itself, so they are not listed as a thread or an agent.
- **Fidelity.** On the design's own dataset (captured from `swarm-tui.js` plus 9's finished
  Docs Site), the panel matches 9e's render (`tests/fixtures/design_9.json`) cell for cell:
  characters, colours and bold, in eight animation frames, plain and with turn 6's unseen
  marks (`tests/test_tiles.py`). Blank cells are compared by background only.
- **Focused or not** (design turn 10, `docs/design/swarm-focus.js`). The panel shows whether
  panorama's pane has the keyboard. Two sources, the latest wins: Textual's app focus / blur
  (the terminal's focus reports, which herdr passes to the focused pane), and herdr's own
  `pane_focused` events (panorama's pane focused, or another one). The terminal report did
  not always arrive, e.g. for the open key's `pane.focus` (the operator's report). When the
  event stream connects, herdr's snapshot sets the start state (a panorama the startup hook
  opened without the keyboard looks unfocused), and when a dock or follow gives panorama a new
  pane id, the last reported focus is judged again against it. Purely
  visual: nothing else depends on it.
  - **`panel_colors = "fixed"`** (the default; the operator tuned these live): `#14161b`
    focused, `#101215` (panorama's original background) not; panorama's own text colours.
  - **`panel_colors = "herdr"`**: herdr's theme. Focused: the unfocused background moved
    `focus_tint` (default 0.15; the full `active_row_bg` was far too light) of the way toward
    the theme's `active_row_bg`; text the theme's `text`, dim `subtext0`, faint `overlay1`.
    Not focused: its `panel_bg`; dim `overlay1`, faint `overlay0` (one step dimmer).
  - In both, panorama's own `panel_bg` / `focus_bg` (hex, in its config) replace the two
    backgrounds. The panel's own background changes; the docked entry's and the hover's row
    backgrounds stay, their text follows. Tiles, status colours, pulses and anything
    animated stay as in 9e. The 2a graph is unchanged.
  - The theme is read from herdr's `config.toml` (herdr's API has no theme colours), layered
    as herdr layers it: the built-in theme (`[theme] name`, default catppuccin; with
    `auto_switch` its `dark_name`, since panorama's colours are dark), `[theme.custom]`,
    then `[theme.custom.dark]` with `auto_switch`. Re-read when the file changes. The
    built-in themes' six tokens are panorama's own table (`theme.py`) of herdr 0.9.3's own
    values, measured per token (an isolated herdr per theme with a client attached, each
    token overridden by a sentinel colour in turn; the few that share a colour with another
    token, at cell level). Every token is measured. catppuccin's `panel_bg` is herdr's
    `#181825`, not the palette's base (the operator: follow herdr).
    `theme.panel_colors` is the one place that turns them into the panel's colours. The `terminal` theme (the host's ANSI palette, which panorama cannot read) and
    a `reset` background fall back to panorama's own colours, `#171a1f` focused.
  - Panorama's status colours stay its own; theming them is a possible follow-up.
  - Golden: `tests/test_focus_theme.py` against `tests/fixtures/design_10.json` (the
    operator's theme and tokyo-night, focused and not, four frames).
    `docs/screenshots/dock-app.png` / `dock-design.png` (focused) and `dock-app-unfocused.png`
    / `dock-design-unfocused.png` show the app and the design side by side.
- **Interaction.**
  - The panel is one column: ↑/↓ move between the entries and `+ New agent`; → does nothing.
  - Enter on an entry docks panorama next to that orchestrator. **One click** on an entry
    does the same at once: it selects the entry and runs exactly what Enter runs. This is
    the operator's request for the side panel; the full 2a graph keeps select-first.
    - On the already-docked entry, the click (like Enter) docks "already there" and focuses
      its agent's pane.
    - **Arrows switch the chats** (`dock_on_arrows`, default true; the operator's proposal
      after #14). In the docked panel ↑/↓ dock next to the highlighted orchestrator once the
      arrows rest for `ARROW_DOCK_SECONDS` (0.2 s, so holding an arrow docks once, at the
      last entry), through the same keep-focus path as below; Enter (like `g`) goes into that
      agent. Not for the docked entry, `+ New agent`, or an orchestrator on another machine
      (Enter keeps its hint). An arrow dock that comes due while a dock runs is looked at
      again when it ends; none happens while a follow runs or once the keyboard has left
      panorama (the terminal's blur, or herdr reporting another pane focused). A click still switches at once. Off: Enter docks as below. It needs
      `dock_keep_focus`: an arrow never moves the keyboard into an agent.
    - **Keep focus** (`dock_keep_focus`, default true; the operator's request: browsing the
      chats took the open key each time). On another entry, Enter and the click dock next to
      it and leave the keyboard on panorama, so ↑/↓ and Enter browse the chats one after
      another. The dock's swap takes panorama as its source, then panorama focuses the
      agent's tab and its own pane there, the same sequence as the open key. The swap's own
      focus is not enough: herdr 0.9.3 logs the new tab as focused, but an attached client
      keeps showing the tab panorama left and moves focus back on its next input (the
      operator's report: Enter seemed to close the panel, and the open key then finished the
      switch; not reproduced with an isolated attached client). Like follow, it looks again
      first: if the operator went to another tab meanwhile, they are left there. If the tab
      cannot be focused, the agent is (as without keep focus). With `PANORAMA_TRACE` set,
      each step records where herdr's focus is, to catch the revert next time. If panorama could not move, the
      agent is focused as before. `g`, and Enter on the docked entry, go into the agent.
      Follow sees the operator in panorama and does nothing, so the two never loop. The full
      2a graph is unchanged: Enter there goes into the agent.
    - **The blink.** Switching chats shows one in-between frame. Measured on an isolated
      herdr 0.9.3 with a real client attached in a pty (its output through a terminal
      emulator, every distinct screen recorded, the real panorama TUI in the moving pane;
      three runs each):

      | order | in-between frame | final frame |
      |---|---|---|
      | move, swap, ratio, tab focus, pane focus (before) | the chat being left, without the panel, at 9–11.5 ms | 28.5–29.8 ms |
      | the same over the socket | same, at ~8.6 ms | ~28 ms |
      | move at the final ratio, then swap (no ratio step) | same, at ~8.6 ms | ~27.5 ms |
      | **tab focus first**, then move and swap (now) | the chat asked for, without the panel yet, at 8.2–9.5 ms | 26.1–28.2 ms |

      herdr renders the first change at once, then about 20 ms later; it has no batch call
      (`layout.apply` makes new panes, it cannot move one), and panorama's pane must leave a
      tab before it appears in another, so one frame stays. Focusing the agent's tab first
      (explicit docks only, and only if at lock time the operator is still on panorama or in
      its tab; a follow is already there) makes it the target chat, shown about 20 ms sooner. Panorama itself never drew a wrong-width or blank panel in between (the
      client shows its pane only at the final width), so holding its repaint would change
      nothing; the socket and ratio-first orders changed no frame, so they were left out.
    - The click is armed like Enter: right after a refresh changed the selected entry, it
      may select but does not dock.
    - It acts on the identity drawn under it, re-checked against the model, and the
      per-cell arrival rule applies unchanged.
  - A click on `?N` answers it instead, and `a` answers the selected entry.
  - Each entry, including the docked one's thread list, is a click target for its
    orchestrator's identity. The per-cell arrival rule, the arming delay, identity re-checks
    and follow all apply unchanged.
- **Extensions the design does not show.**
  - 3a has no separate "selected" look. The keyboard cursor wears the design's hover look:
    the darker background and a dim `›`. On the docked entry, its gold bar wins.
  - A `refresh failed` chip appears after the header when a refresh fails.
  - `a answer` is left out of the footer when there is no inbox.
  - `z graph` sits on the footer's rule (see Follow).

## Refresh strategy

A background worker rebuilds the model every 1.5 s; the view animates from the
last render (see Smooth animation) and re-renders only on a refresh or input. Collects never overlap: a tick that finds the previous
collect still running is skipped (a Textual `exclusive` worker only marks the
old thread cancelled, it keeps running), so one collect at a time owns the
tail offsets. A failing collect keeps the last good model on screen and the
header shows a `refresh failed` chip with the reason (called `stale` before turn 8).

- `herdr api snapshot`: one call.
- Per session, every file is stat-ed. Unchanged files cost nothing; changed
  ones are read from their offset (see Tailing). In the appended bytes, the
  lines worth a JSON parse are found with one C-level search per marker over
  the whole buffer (spawn and SendMessage calls, acknowledgements, resumes,
  workflow launches, notices, inbox receipts, titles, string user messages).
  The rest are only counted for `"type":"tool_use"`.
- Agent files untouched since before the done window and never read are
  skipped until they change: they cannot affect anything the view shows. Except (turn 8) an
  agent that never ended (no notice, no completed acknowledgement): it may still be waiting on
  its own background Bash, so its file is read after all when it holds a background launch
  (one byte search per file version).
  Main transcripts are always read in full.
- Measured on this machine (7 live sessions, 48 MB largest main transcript,
  175 agents in one session): first load 0.8 s in the worker, later ticks
  5–7 ms. A full scan of all agents of the heaviest session (the accuracy
  gate) takes 2.5 s.
- Inbox: one read-only query per tick.

`events.subscribe` would make `!` instant (`pane.agent_status_changed`), but
transcripts change far more often than herdr events and need polling anyway.

## Configuration

`config.toml` in the herdr plugin config dir (`$HERDR_PLUGIN_CONFIG_DIR`, else
`herdr plugin config-dir leavemyyard.herdr-panorama`), re-read when it
changes; the same pattern as herdr-inbox's `tab_prefix`:

```toml
done_window = "1h"   # how long finished threads stay visible: seconds or "90m", "2h30m"
stale_after = "12h"  # an orchestrator idle this long reads "stale" (PANORAMA_STALE_AFTER overrides)
home_tab = true      # the home tab (false: the key opens the popup)
cockpit = false      # experimental: panorama docked left of the selected agent (see Cockpit)
cockpit_width = 46   # the list's drawable columns in the cockpit (36..119)
cockpit_follow = true # re-dock beside the orchestrator the operator switches to (see Follow)
cockpit_follow_tabs = "any" # "single": follow only into tabs whose one pane is the orchestrator
machines = "all"     # saved herdr SSH machines to show: "all", "none", ["label", ...] (docs/REMOTE.md)
highlight_unseen = true # pulse what changed since the operator looked (see Unseen)
animation_fps = 20   # the colour tick, 2..30 (see Smooth animation); read when the view starts
```

Default 1 hour. `PANORAMA_DONE_WINDOW` (seconds or a duration) overrides it.
Done threads that ended earlier are dropped; the rest fold into the
"✓ n complete" card.

## Safety rules

Learnt in the herdr-inbox reviews, applied here:

- **Read-only towards Claude and herdr.** The only herdr calls that change
  anything are `agent focus <pane>` and `plugin action invoke …inbox.open`,
  both on an explicit key press, plus the home tab, New agent, and, with
  `cockpit = true`, the cockpit's moves of panorama's *own* pane (`pane move`,
  `pane swap`, split ratio, `pane.focus`; see *Cockpit*). Agents' panes never move. Nothing is typed into a pane except the New agent prompt.
  The inbox DB is opened `mode=ro`.
- **Every herdr call** goes through one wrapper with a timeout (3 s) that never
  raises into the UI; every argument passes the control-character sanitiser.
- **No background event changes what the next key does.** Cursors are target
  identities (orchestrator `pane_id`, thread id, agent id), never row indexes.
  After a refresh the selection is re-resolved by identity; a vanished target
  parks the cursor on a placeholder (keys act on nothing) until the operator
  moves.
- **Arming**: action keys (`g`, `a`, `enter`) are ignored for 0.4 s after the
  selected target was (re)shown by something the operator did not cause (first
  paint, a refresh that changed the selected card), so a key meant for the old
  card never acts on a new one. The arming key holds everything those keys
  read: the selected orchestrator, its herdr status and open inbox questions,
  whether the inbox exists, the thread, the agent, its status, questions and
  transcript.
- **Parked cursor**: when the focused column's selection vanished, action keys
  act on nothing until the operator moves.
- Text from transcripts is untrusted: control characters are stripped before
  it is painted; it is never executed or sent anywhere.

## Limitations (PoC)

- Claude Code only; agents of other harnesses in herdr are not shown.
- Thread heuristics as listed above; resumes of an agent are spans, but a
  long-lived agent reused across two PRs is drawn in the thread of its first
  description.
- Agents started from a session that has since been `/clear`ed belong to the
  old session id and are not shown (the pane now reports the new one).
- Workflow agents queued but not started are invisible (the journal has no
  queued event); `q` is used only by the explicit channel.
- A background agent that dies without a notice (Claude Code crashed) shows
  as running for up to 10 minutes of silence, then as `done?`. From the
  transcripts alone, a crash and a quiet finish look the same. This is a known
  issue ([#2](https://github.com/LeaveMyYard/herdr-panorama/issues/2)). With
  the opt-in hook installed, such an agent is shown as `lost` once its Claude
  process is gone, unless the session was resumed since or the process cannot
  be identified reliably.
- Status windows (120 s, 10 min, 1 h) are heuristics; a model turn longer
  than 10 minutes without a line briefly reads as done until the agent
  writes again.
- `t transcript` only shows the path.
- Times are local; durations use the transcript timestamps (UTC).
