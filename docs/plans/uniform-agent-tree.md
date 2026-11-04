# Plan: one uniform agent tree, agents titled by their spawner's name

Status: **Approved 2026-11-04, D1-D8 as recommended.** Operator, verbatim: "Approving all recommendations". The V11 design (commit `a48bde2`, `docs/design/herdr-app/state-refresh-v11/`) is approved too. This document is planning only; it changes no code, installs nothing and edits no configuration.

Branch `plan/uniform-agent-tree`, from `origin/app/herdr-app-mod` at `0adfb5f` (PR #156 merged).

- Evidence was read at `fb3ec92` and checked against `0adfb5f`. Between them only the V10 state vocabulary landed: `app/shared/state.js`, glyph SVGs, bundle lists, `scripts/app-build` and `app/installer/manifest.py`. Every file cited below except `scripts/app-build` is byte-identical.
- The companion design is V11. Its source is `app-design-state-refresh` at commit `a48bde2`, `docs/design/herdr-app/state-refresh-v11/README.md`, which the operator is reviewing separately.
- Paths are relative to the repo root. `KIT` is herdr-agent-kit v0.1.1, pinned in `Cargo.lock` to `c606b48`.

## 0. Operator decisions (verbatim, 2026-11-04)

> "Graph should just treat all agents and their subagents like a graph and different layers should not have any special names or components"

> "Can we just ask each agent to name itself when starting? Then no threads or anything. And names would come from agents of any level, right?"

> Root sessions: "For root session let's use 1 and let 3 override it. 2 I don't think we really need."

What the options mean:
- **1:** the harness's own session title.
- **3:** the operator's herdr tab or pane name.
- **2:** rejected. It was a `set_name` MCP tool plus a startup instruction.

The answer to "names would come from agents of any level" is **yes, with no agent cooperation.**
- **Claude.** Every Agent/Task spawn carries a `description` that the spawner writes, at every depth. Panorama already reads it.
- **Codex.** Every spawned thread has its own `thread/read` record at every depth. One caveat, covered in §2.2: the field Panorama reads today is the thread's `name` or Codex's `agentNickname`. It does not read the spawner's own task text.

---

## 1. Current state, with evidence

### 1.1 The model hard-codes orchestrator → thread → agent

`crates/panorama-core/src/model/`:

- **The module doc** (`mod.rs:1`) says it outright: "The swarm model: orchestrators, the threads they drive, and the agents in those threads."
- **`Swarm.orchestrators: Vec<Orchestrator>`** (`swarm/mod.rs:164`). An `Orchestrator` (`swarm/mod.rs:92-143`) is a herdr pane with:
  - `tab_label` (`:100`), the only root name;
  - `flavour` (`:111-112`), derived from its threads;
  - `threads: Vec<Thread>` (`:123-124`);
  - `hidden_done` (`:125-127`), "Done threads older than the window".
- **`Thread`** (`swarm/mod.rs:57-82`) has:
  - `id` and `name`;
  - `source: ThreadSource`, one of explicit / workflow / ref / turn / session (`swarm/codes.rs:92-106`);
  - `phases: Vec<Phase>` (`:39-55`, `PhaseState` at `codes.rs:108-122`);
  - `status: ThreadStatus`, one of active / done / failed / cancelled (`codes.rs:78-90`);
  - `worktree`, `pct` and `flavour`.
- **Agents exist only inside a thread.** `Orchestrator::agents()` flattens `threads[].agents` (`swarm/mod.rs:153-158`). `Agent` (`swarm/agent.rs:9-66`) carries:
  - `phases: Vec<i64>`, indexes into its thread's phases (`:47`);
  - `parent`, where `None` means "a thread's own agent" (`:48-50`);
  - `depth` (`:51-52`).
- **Agents have no title field.** The label candidates are `description` (`:13`), `role` (`:15`) and `agent_type` (`:17`).
- **`Counts`** (`swarm/counts.rs:9-25`) has `orchestrators`, `threads` and `active_threads`.
- **`TreeKids::of(thread)`** (`tree.rs:19-58`) builds the spawn tree inside **one thread**. Agents whose parent is outside the thread become the thread's own.
- **The same types travel unchanged over three wires:**
  - panel protocol v1, as `state.swarm` (`docs/protocol/panel-v1.md:82`): "`model.Orchestrator` fields, with `threads[]` (`model.Thread`, with `phases[]` and `agents[]`)";
  - `panorama snapshot --json` (`src/snapshot/json.rs:61-140`);
  - app feed v1 (§1.5).
- **The Rust model goldens are byte-for-byte Python output** (`tests/model_goldens.rs:1-3`, from `tools/capture_goldens.py`). Any model change breaks Python parity on purpose (§8, R3).

### 1.2 The collectors invent threads

**Claude** (`src/collect/`):

- **Grouping.** `threads/mod.rs:1-6, 122-175` runs the sources, strongest first. An agent belongs to the first source that claims it:
  1. the explicit channel: `threads/declared.rs:13-73`, id `x:<thread>`, agents matched by id or by a description glob (`:39`);
  2. native Workflow runs: `threads/workflow.rs`, whose phases come from the run summary or journal;
  3. PR/issue references in descriptions: `threads/roles.rs:119-135` (`ref_key`), `threads/heuristic.rs:53`;
  4. operator-prompt eras, named by the first agent's description plus "+N": `threads/heuristic.rs:104-111`, using `prompt_before(...).index` at `threads/mod.rs:152`;
  5. orphans, named by the root agent's description: `threads/mod.rs:95-104`.
- **Heuristic phases.** `threads/phases.rs` and `threads/spans.rs` build phases from launch and resume spans.
- **Thread status.** `threads/finish.rs:9-29`: active while any agent is live and not failed; failed if the newest agent failed; cancelled if all were; else done.
- **The thread done-window** is at `threads/mod.rs:161-173`:
  - it hides done **and failed** threads whose `ended < now - window`, and counts them into `hidden_done`;
  - so today a failed thread disappears after the window as well.
- **The agent done-window already exists.** `agents.rs:63` drops a finished agent (`!status.is_live()`) older than the window. `agents.rs:201-237` keeps every ancestor of a shown agent, so there are no holes in a chain.
  - **Note:** `Status::is_live()` (`crates/panorama-core/src/model/status.rs:147-152`) counts `Failed` as live. Without the thread window, a failed agent would **never** age out. See §2.4 and §8, R5.
- **The "main card".** `threads/main_card.rs:1-73` builds a synthetic thread for the orchestrator itself, holding a `main:` agent, when the session is blocked or has inbox questions no subagent filed. It is named after `o.tab_label` (`:61`).
- **Flavour** is derived from thread sources (`threads/finish.rs`) and stored on the orchestrator (`orchestrator.rs:207`).
- **The explicit channel** is read per session at `orchestrator.rs:180-186`, from `src/collect/events.rs:24-90`.

**Codex** (`src/collect/codex/`):

- **The root.** One pane is one `Orchestrator`. Its root agent is `main:codex:{pane}` with role `orchestrator` (`mod.rs:77, 93-96`).
- **The default thread.** All agents sit in a session thread `codex:{root_thread_id}` with source `Session` (`mod.rs:131-150`).
- **Phases** are the root's turns, named by their prompts (`history.rs:32-44`).
- **Declared threads.** Per-thread `report_thread` channels (`channels.rs:14-18, 61-85`) become extra threads (`declared.rs:20-89`). Claimed agents leave the session thread (`mod.rs:104-113`).
- **The tree is already arbitrary-depth.** Each child is proved by `thread/read` `parentThreadId` (`src/codex/history.rs:26-27, 162-172`). Chains are ordered at `tree.rs:29-78`, and the root maps to `main:codex:{pane}` at `tree.rs:10-27`.
  - Coverage is capped at 8 thread reads per refresh (`src/codex/discovery.rs:8`, `history.rs:146-148`). Wide trees fill in progressively.

### 1.3 The TUI has orchestrator and thread columns, cards and keys

`crates/panorama-core/src/view/`:

**Graph, wide**
- Columns are `orchestrators | threads | depth 1 | depth 2 …`:
  - `graph/wide.rs:1-3, 32-36`: `vec![ow, tw, dw]` and `[vec![ow, tw], vec![dw; depth]]`;
  - `graph/layout.rs:12` (`tier`, three widths);
  - `graph/layout.rs:63-69` (`col_label`: "orchestrators", "threads", "depth N");
  - gaps are fixed per column index in `Columns::place`.
- `graph/orchs.rs` draws the orchestrator column.
- `graph/threads.rs` draws the thread column. It contains:
  - a background card for the orchestrator's own background tasks;
  - the active threads;
  - the "✓ n complete" card for done threads.
- `graph/thread_card.rs` draws the thread card. It shows the phase glyph, the current phase with its duration, a pod row and an age tag. When selected, it unfolds into the phase steps: `cards/phases.rs:15-131` (`cur_idx`, `phase_glyph`, `thread_glyph`, `phase_sub`, `step_line`).
- `cards/pods.rs:1-29`: `top_only` draws "the thread's own agents and `+N nested`".

**Agent cards are not titled by `description`.**
- `graph/agent_card.rs:138-148` titles a card `role_label · short_id · model`.
- `role_label` (`agent_text.rs:6-12`) picks role, else agent type, else "agent".
- No view in `crates/panorama-core/src/view` reads `Agent.description`. The only `description` reads are for background tasks (`cards/details.rs:17`, `dock/items.rs:130`).
- So Panorama already collects the spawner's name but never shows it.

**Selection and targets**
- `View` (`state.rs:21-66`) holds:
  - `thread: Option<String>` (`:25`), including the `done:` and `bg:` pseudo-ids (`:12-15`);
  - `focus`, where 0 = orchestrators, 1 = threads and 2.. = depth (`:28-29`);
  - `done_open` (`:31`) and `remember_thread` (`:35`).
- `Target` (`target.rs:16-46`) has these thread-scoped variants:
  - `Done(pane)`, `Thread{pane, thread}` and `Fin{pane, thread, group}`;
  - `AgentRef{pane, thread, agent}`.
- `nav.rs:11-60` holds `split_threads`, `find_thread` and `find_agent(thread, id)`. `nav/tree.rs:95-194` holds `tree_entries(t: &Thread)`, `path_ids(t)` and `parent_key(t)`.

**Dock (46 columns)**
- `dock/state.rs:12-85` gives `DockState` its `active` and `done` thread lists. It also has a Codex-specific filter for the session thread (`:45-64`).
- `dock/tiles.rs:59` (`thread_tiles`) draws line 2: one tile per active thread.
- `dock/items.rs:19-33` defines `Item::Thread` and `Item::Spawn` rows. `dock/tree_rows.rs:8` cuts the spawn rows at a 64-level guard. Leaf agents get no rows.

**Narrow (< 120 columns)**
- `narrow/card.rs:1-216` draws thread cards with phase lists and a "✓ n complete" card.

**State**
- `orch_state.rs:175-222` computes an orchestrator's state by walking threads. `codex_root` (`:217-222`) recognises the Codex session-thread root agent.

**Root titles**
- `harness.rs:18-31` titles the root card `<harness> · tab_label`.

### 1.4 Where names come from today

| Node | Harness | Read today | Where |
|---|---|---|---|
| Subagent | Claude | `meta.json` `description`, else the spawn's `input.description`; `agentType`, else `input.subagent_type` | `src/collect/agents.rs:72-79`; KIT `fold/stores/spawns.rs:61-63` (`SPAWN_TOOLS = [Agent, Task]`, KIT `wire/tools.rs:5`) |
| Subagent | Codex | `thread.name`, else `thread.agentNickname`, else the raw thread id; role from `thread.agentRole` | `src/codex/parse.rs:42-45`; `src/collect/codex/history.rs:77-86` |
| Root | Claude | herdr tab `label` (inbox mark stripped), then `custom-title` (`/rename`), `agent-name`, `ai-title`, herdr `terminal_title_stripped`, pane id; `· <pane>` added when one tab holds several agents | `src/collect/orchestrator.rs:79-108`; KIT `view.rs:12-38`, `wire/line/classify.rs:47-59`. The Python twin is `collect.py:176-178`. No `summary`-type record is read. |
| Root | Codex | The card shows `tab_label`, from the same `label()` with no session. The session thread's name and the root agent's description prefer `thread.name` over `tab_label` | `orchestrator.rs:135`; `collect/codex/mod.rs:101-102, 134-142` |
| Root, app | both | Stock `agent.space_name`, else the herdr-app tab name when `tab.named` (otherwise a "telling" live pane title), else the address. Unmatched feed roots use `tab_label` | `app/shared/model.js:4-9, 21-23, 57` |
| Root, app core feed | both | No tab or pane name exists. `hc ls --json` rows have no tab id or label; the core snapshot is built with `"tabs":[]` | `docs/protocol/core-host.md:13-18`; `src/host/source.rs:85, 127-129` |

Fallbacks that exist but are not used for titles:
- Claude first prompts are used only as a turn-grouping index (`threads/mod.rs:152`).
- Codex turn prompts and summaries become phase names and `last_message` (`collect/codex/history.rs:39-45, 93-100`).

### 1.5 The app feed v1 schema and the app mods

**Schema** (`docs/protocol/app-feed-v1.md`)
- Top level (`:44-52`): `v, generated_at, herdr_ok, inbox_ok, errors, counts, orchestrators`.
- `Orchestrator` (`:54-80`) includes `threads` (`:74`) and `hidden_done` (`:75`). `tab_label` is a "legacy source title" (`:63`).
- `Thread` (`:82-96`) and `Phase` (`:129-137`) are their own sections.
- In `Agent` (`:98-127`), `phases` (`:120`) are thread phase indexes. `parent` (`:121`) is documented as "null means thread child".
- `Counts` (`:148-163`) has `orchestrators`, `threads` and `active_threads`.
- The prose builds on threads throughout:
  - the thread key (`:169-172`);
  - dangling and cyclic nodes becoming thread children (`:203-208`);
  - the thread mark and the empty-thread fallback (`:245-248`);
  - an empty failed thread counting as needs-you (`:252-256, 280-281`);
  - folding by thread status (`:264-265`);
  - graph columns from "the selected thread's spawn tree" (`:290-293`);
  - Peek's thread name (`:220`).

**Validation**
- Every listed field is required (`:21-24`). Unknown fields are ignored. "Changing a type or meaning needs another version" (`:29-30`).
- In the client: `app/shared/contract.js:20-23` requires every key. `:35` throws unless `feed.v === 1`. `:39-53` checks:
  - thread ids are unique;
  - agent ids are unique per root across threads;
  - `agent.phases` stay in range.
- On any throw, `readResult` (`:57-64`) keeps the previous feed and marks it `stale`.

**Serializer**
- The feed reuses `src/snapshot/json.rs` (thread `:71-85`, phases `:61-69`, counts `:116-118`) through `src/app_feed/serialize.rs:105`.
- `serialize.rs` checks thread-id uniqueness (`:66-71`), checks per-root agent uniqueness (`:67-73`), and walks `threads[].agents[]` to add `harness` (`:123-131`).
- `src/app_feed/mod.rs:49` uses `root.agents()`.

**Mods** (every shared file is copied into each mod by `scripts/app-build`; `--check` fails on stale copies)
- **`app/shared/tree.js`**
  - `aggregate` walks `root.threads`, with the empty-failed-thread rule (`:30-37`).
  - `threadMark` (`:39-42`).
  - `rows()` (`:60-98`) produces thread rows, the "✓ N tasks done" fold of done threads, and `forest(thread.agents)`.
- **`app/shared/contract.js:27`**: `threadKey`.
- **`app/shared/model.js`**
  - Synthetic roots get `threads: []` (`:17, 32`).
  - `rootSignature` (`:79`).
  - `harnessOf(agent, root, thread)` (`:80-88`).
- **`app/shared/peek-caller.js:19-27`** re-resolves `root.threads.find(...)`, then the agent. `app/shared/routing.js:6` carries `thread`. `app/shared/marks.js:26-29` holds the phase-bar renderer.
- **`panorama` (sidebar)**
  - `main.js:65, 145`: child count = threads + native children.
  - `:85-95`: summary, thread and agent rows.
  - Agent rows are titled by `agent.role` (`:96`).
- **`panorama-graph`**
  - `graph.js:100-114`: column 1 is `threadNodes`, each card `64 + phases*20` tall. The tree comes from one selected thread. `nativeFallback: !thread` is already a depth-only layout.
  - `main.js:22`: the state is `orch/thread/agent`.
  - `main.js:192-195`: the thread name sits in the breadcrumbs. Column labels are `'Orchestrators' | 'Threads' | Depth n`.
  - `cards.js:46-56`: the thread card. `cards.js:62-63`: agent cards titled `role`, then `id · model`.
- **`panorama-peek`**
  - `subject.js:12-16` resolves thread, then agent.
  - `main.js:122-123`: the title is `agent.role || agent.description || root.tab_label`. The subtitle is the thread name.
  - `:132`: "Open orchestrator".
- **`panorama-pane`**: `space.js:9-11` and `main.js:63` show "N orchestrators · N threads · N agents".
- **`panorama-spaces` and `panorama-tabs`** depend on threads only through `aggregate`.

### 1.6 The explicit thread channel and the Claude plugin

- **SessionStart instruction**
  - `src/hook/session.rs:19`, `SESSION_TEXT`: "…call report_thread when a thread starts and whenever its phase changes…". It is sent once per session, inside herdr only (`:84`, `:153-191`).
  - The Python twin is `src/herdr_panorama/hooks.py:44`.
- **MCP**
  - `src/mcp/server.rs:22` holds the instructions. `:133` dispatches the only tool. `src/mcp/tool.rs:10` names it `report_thread`.
  - Arguments: `thread_id, title?, phase?, phases_done?, next?, status?, agents?` (`tool.rs:22-125`). Calls are parsed and limited at `call.rs:267-376`. Results are written through `src/report/{emit,merge}.rs` to `events/<sid>.jsonl` (`src/collect/paths.rs:17-38`).
  - Session resolution: Codex `_meta.threadId`, then herdr `pane get`, then `CLAUDE_CODE_SESSION_ID`.
  - The Python twin is `mcp_server.py`.
- **`panorama emit`**: routed at `src/main.rs:33`, implemented in `src/emit/**`. Help is in `src/emit/help.txt`; it writes the same file.
- **Manifests**
  - `.claude-plugin/plugin.json` and `marketplace.json` both say the plugin "gives orchestrators a report_thread MCP tool". The plugin declares one MCP server, `scripts/mcp`.
  - `herdr-plugin.toml:5` says "the threads they drive".
  - `codex/config.example.toml:17-22` registers `panorama mcp` for Codex.
- **Contract.** `src/contract.rs` defines `CLAUDE = 1`, which covers "the MCP tool's name, text and schemas, and the events format (v1)". It is checked by `hooks/run-hook.sh` and `scripts/mcp`.
- **Config.** `done_window` (`src/settings/durations.rs:11-12, 59-62`; README `:315, 347`; DESIGN `:1953-1975`) is the only thread-related key. The env variables `PANORAMA_EVENTS_DIR` and `PANORAMA_DONE_WINDOW` also exist.
- **Docs.** `docs/DESIGN.md:398-480` (Threads, Explicit channel) and `:571-605` (SessionStart instruction, `report_thread`). README `:5, 179-181, 280-281, 296-299, 315` and `app/README.md` cover the same ground.

---

## 2. Target data model and naming rules

### 2.1 Model

```text
Swarm { roots: [Root], generated_at, herdr_ok, inbox_ok, errors, stale_after }
Root  { pane_id, host, terminal, harness, tab_id, tab_no, workspace_id, focused, cwd,
        session_id, transcript, herdr_status, pending, inbox, inbox_texts, bg,
        last_activity, machine, machine_id,
        title, title_source,            # §2.2
        agents: [Agent],                # flat, parents before children
        hidden_finished }               # agents dropped by the done-window (§2.4)
Agent { id, title, title_source, description, role, agent_type, model, status,
        started, ended, last_at, tool_calls, activity, last_message, questions,
        pending, question_ids, error, parent, depth, resumes, inferred,
        transcript, seq, bg, harness }
```

Rules:
- **One kind of node below the root.** `parent: null` means a direct child of the root. Otherwise `parent` is another agent of the same root. A dangling or cyclic parent becomes a root child (today's `TreeKids` rule), so no agent is ever hidden.
- **`depth` is recomputed from the parent chain,** root children are 1. The value stored from Claude's `spawnDepth` becomes advisory.
- **The root is the depth-0 node.** Its own status (herdr status, pending prompt, inbox questions no agent filed, its background tasks) is on the root itself. This replaces the synthetic main-card thread (`threads/main_card.rs`) and the Codex `main:codex:*` root agent (`collect/codex/mod.rs:77-96`).
- **These are deleted:**
  - `Thread`, `Phase`, `ThreadSource`, `ThreadStatus`, `PhaseState`;
  - `Agent.phases`;
  - `Orchestrator.flavour`, `threads` and `hidden_done`;
  - `Counts.threads` and `Counts.active_threads`.
- **`Counts`** becomes `{roots, agents, running, waiting, questions, blocked, failed}`.
- **`role` stays as secondary text** (expanded cards and Peek, per V11). The heuristic roles (`threads/roles.rs`, `role_of`) stay for that use only. Workflow labels may still feed `role`.
- **`TreeKids::of(root)`** groups `root.agents` by parent across the whole root. Both TUI and app share these semantics.
- **"Having a pane only changes click behaviour."**
  - A node with a verified terminal (every root, plus app-native terminal children) focuses that pane.
  - A transcript-only agent opens Peek in the app and selects/details in the TUI.
  - Card structure is identical in both cases.
- **Interim names.** The Rust type names `Orchestrator` and `orchestrators` are renamed `Root` and `roots` in slice U6. Until then the old names carry the new meaning.

### 2.2 Naming precedence

Every title is cleaned before it is stored:
- drop control and bidi characters, collapse whitespace, trim;
- strip leading non-alphanumeric decorations, as `model.js:3` does;
- cut to 80 characters; the views truncate further.

`title_source` records which rule won: `spawner`, `prompt`, `type`, `session`, `tab`, `pane` or `id`. It drives no logic. It exists only so tests and Peek can say where a name came from.

**Subagent, Claude** (any depth; Workflow-run agents included):
1. The `agent-<id>.meta.json` `description` (`agents.rs:72-75`).
2. The spawn's `input.description` (KIT `spawns.rs:61`). Claude Code requires this parameter, so it is almost always present.
3. The first line of the subagent's first prompt, truncated. The prompt is the spawn's `input.prompt`, or the first user record of the subagent transcript. The kit stores the spawn input but not the prompt text today: this needs a kit accessor, or a read of the first record through the existing scanner. The open question is noted in §8.
4. `agent_type` (`Explore`, `general-purpose`, …) plus the short id: `Explore a1b2c3d`.
5. `agent <short id>`.

**Subagent, Codex** (any depth):
1. The spawner-given task name, **if the spawn item carries one.**
   - Today `collabAgentToolCall` items are reduced to `tool` and `receiverThreadIds` (`src/codex/parse.rs:80, 129-140`). Their arguments are deliberately discarded (`parse.rs:1`).
   - Slice U1b adds this field after capturing a real item and pinning it as a fixture. **Unverified**: the repo contains no captured spawn arguments.
   - **U1b finding:** the codex-cli 0.166.0 app-server schema shows the spawn item carries the spawner's whole `prompt`, not a name. The spawner-given `task_name` is on the child's own `thread/read` record, as the last segment of `source.subAgent.thread_spawn.agent_path`; U1b reads it there (`docs/codex.md`, "How Codex agents are named").
2. `thread.name` (`parse.rs:42`), when non-empty.
3. `thread.agentNickname` (`parse.rs:43`). This is Codex-assigned, not spawner-given; see §8, D2.
4. The first line of the thread's first turn prompt, truncated. It is already parsed as `turn.prompt` (`parse.rs:108-119`).
5. `agentRole` (`parse.rs:44`) plus the short thread id, else `agent <short id>`.

**Root, Claude**:
1. **The operator's herdr name**, the "3" override:
   - the pane name if the operator set one;
   - else the tab label (inbox mark stripped, `text.rs:124`) when it is operator-set.
   - A label counts as **not operator-set** when it is empty, all digits, equal to the agent kind or program name (`claude`, `codex`), or equal to the label Panorama itself writes back (`src/home/label.rs:22-40` `neutral_label`: agent name, pane name, agent kind, or `tab`). This is the TUI counterpart of the app's `tab.named` and program-name check (`model.js:4-8`).
   - When a tab holds several agent panes, the tab label applies to none of them. The pane name or the session title is used, and today's `· <pane>` suffix (`orchestrator.rs:104-105`) goes away.
2. **The session title**, the "1" source: `custom-title` (`/rename`), then `agent-name`, then `ai-title` (KIT `view.rs:34-38`, consumed at `orchestrator.rs:88-92`).
3. The first line of the first operator prompt, truncated (KIT `Conversation::prompts()`).
4. herdr `terminal_title_stripped`, when "telling" (the `model.js:7` rule: not a path, not `user@host`, not the program name).
5. `Claude · tab N`, else `Claude · <pane>`.

**Root, Codex**:
1. The operator's herdr name, with the same rule as Claude.
   - This **reverses** today's preference for the Codex name in the session thread and root agent (`collect/codex/mod.rs:101-102, 137-140`). The card already shows `tab_label`, so the visible change is small.
2. `thread.name` of the root thread (`thread/read`, `history.rs:22-24`), which Codex sets from `/rename` or its own title.
3. The first line of the root's first turn prompt.
4. The telling terminal title.
5. `Codex · tab N`, else `Codex · <pane>`.

**Root in the app** (herdr-core host): the core writer has no tab or pane data, so the rules split between writer and mod.
- The writer emits `title` from rules 2–4 only, with `title_source` ∈ `session | prompt | pane | id`.
- The mod applies rule 1 from live app data, in this order:
  1. a pane name the operator set;
  2. the tab name when `tab.named` is true and the tab holds exactly one agent root;
  3. the feed `title`;
  4. the existing telling live pane title;
  5. the program name and address.
- `agent.space_name` currently wins over everything (`model.js:23`). See §8, D3.

### 2.3 Status aggregation (unchanged vocabulary, new scope)

- **A node's own state:**
  - a root: herdr status, pending prompt, inbox questions no agent filed, background work;
  - an agent: its status.
- **A subtree's "needs you" marks** (✗ ! ?N) aggregate over all descendants, with distinct question ids.
- **These rules disappear:**
  - "empty failed thread contributes a root fail chip" (`tree.js:33`; `app-feed-v1.md:252-256`);
  - the thread mark rules and `threadMark`;
  - Codex `codex_root` exclusions.
- **Dock and graph states** use the same `stOf`-style order as today (`dock/state.rs`, `orch_state.rs`) over the root's own state plus its descendants.

### 2.4 What replaces the thread done-window

The window becomes **per-agent**, keeping today's mechanism and its config key:
- **Keep `agents.rs:63` and `agents.rs:201-237`.** A finished agent whose end is older than `done_window` is dropped, unless a descendant of it is still shown.
- **`done_window` keeps its name and default (1 h).** Its documented meaning becomes "how long finished agents stay visible". `PANORAMA_DONE_WINDOW` is unchanged. No new key.
- **Failed agents** age out after the window like done ones, counted from `ended` (else `last_at`). This preserves today's visible behaviour, where failed threads were hidden after the window (`threads/mod.rs:166`). Without this rule they would stay forever, because `Failed` is live (`status.rs:147-152`). See §8, D4.
- **A root never ages out.** It exists while its pane exists.
- **`hidden_finished`** per root counts the agents the window dropped. The views show it as "+N older" in the root's fold. It replaces `hidden_done` and the "✓ n complete" thread card.
- **Folding.** Finished siblings fold into one `N finished · names…` card or row per parent at every depth, root included. This is today's `fin_card` and `FIN_CARD` group mechanism (`graph/fin_card.rs`, `state.rs:16-17`), with the root as just another parent.

---

## 3. Feed protocol: v1 → v2 (recommended), not additive

**Why not additive.** Three things rule it out:
- The v1 fields `threads`, `phases`, `hidden_done` and the thread counts are **required**. `parent: null` is documented to mean "thread child" (`app-feed-v1.md:21-30, 121`).
- Dropping these fields, or changing `parent`'s meaning, is a version change by the document's own rule.
- An additive route would need a fake single thread per root forever. The v1 contract test would keep enforcing thread uniqueness and phase indexes against meaningless data (`contract.js:39-53`; `tests/app_feed_contract.rs:149-182, 305-344`).

**v2 shape.**
- Top level: `{v: 2, generated_at, herdr_ok, inbox_ok, errors, counts, roots}`.
- `Root` and `Agent` as in §2.1, plus the v1 retained fields (`host`, `terminal`, legacy `tab_id`/`workspace_id`/`focused`/`machine`, per the COMMON 2026-11-03 A1 decision).
- `counts`: `{roots, agents, running, waiting, questions, blocked, failed}`.
- `Thread` and `Phase` sections deleted.
- Kept from v1: the numeric semantics (finite safe integers, `1.0` accepted), the unknown-field rule, the identity rule (host + terminal / host + pane), and the duplicate-root rule.
- The new document is `docs/protocol/app-feed-v2.md`. The v1 document stays, marked superseded. `tests/app_feed_contract.rs` already parses the document's tables (`:7, 12-31`), so the new document is the schema; the section count drops from 7 to 5.

**Compatibility with the installed mod.**
- The live install (receipt v2, generation `8a228fdb…`, COMMON "LIVE INSTALL COMPLETED") pins its six mods and its private binary together:
  - the receipt records the binary's SHA-256 and mode (`app-feed-runtime-v1.md:54-58`);
  - `--ensure` refuses a modified binary (`:207-209`);
  - there is no in-place upgrade (`:47-52`).
- So a v2 binary can only arrive through `uninstall`, then `install`, which also replaces every mod. The two versions never meet in normal operation.
- **Mismatch safety.**
  - If an old mod ever reads a v2 file, `parseFeed` throws at `feed.v`, `readResult` keeps the last v1 feed marked stale, and with no prior feed the mod shows stock agents only (`contract.js:35, 57-64`; `app-feed-v1.md:33-35`). The result is degraded but safe.
  - If a new mod reads a v1 file, it rejects it the same way.
- **The capability probe** changes from `panorama app-feed v1` to `panorama app-feed v2` (`src/app_feed/cli.rs:4`; `app/shared/feed-client.js:159`). An old client refuses to start a new writer, and the reverse holds too. That is the intended pairing, not a regression.

**Generation and receipt lifecycle: unchanged.**
- Receipt v2, `install_id`, `G = data/panorama/runtime/<install_id>`, `control.json`, `writer.lock`, `ownership.json`, the feed-digest journal and the fixed `data/panorama/feed.json` all stay as they are.
- `app-feed-runtime-v1.md` keeps its version, because the runtime contract does not change. Only its "Atomic full feed v1" wording (`:133`) and the probe string (`:216-217`) are edited.
- The approved uninstall-retention exceptions are untouched:
  - `panorama.feed:null` and `panorama.feed.owner:null`;
  - `data/panorama-spaces/store.json` and `data/panorama-tabs/store.json`;
  - `layouts/.state/panorama.json`.
- Their documented shapes are panel, machine and folding preferences, not thread keys (COMMON lines 74, 102-104). U8 must still check that no retained preference key embeds a thread id. Any graph or sidebar fold keys are in memory only.

**Goldens and fixtures.**
- Rewrite `tests/fixtures/app-feed/swarm.json` as v2:
  - Current content: root `core:t2` with the 5-phase "Migration" thread of 13 agents, plus 4 empty threads, and five more roots.
  - New content: root `core:t2` with the same 13 agents as one tree; a root whose only problem is its own `blocked` status; a failed leaf at depth ≥ 3; a fold of finished siblings; `hidden_finished > 0`.
- Delete `empty-failed.json`, whose case no longer exists. Replace it with `own-blocked.json`, the root-only needs case.
- `degraded.json` changes `v`, so update the two SHA-256 values in `tests/fixtures/app-feed-runtime/lifecycle.json:49, 58`, which are pinned to it.
- Rust: `tests/app_feed_contract.rs`, `app_feed_serialization.rs` (`:22, 65, 72, 109`) and `app_feed_runtime.rs` (`:90, 101, 350`, for the key rename).
- Node: every `app/tests/*.test.mjs` listed in §5, plus `scripts/app-build --check`.

---

## 4. TUI changes

The core model and the panel protocol go to **2.0**:
- `state.swarm` carries `roots[]` with flat `agents[]`;
- `PROTOCOL` major 2;
- `contract.rs` raises `PANEL` and `TUI` to 2, with `scripts/panel` and `scripts/tui` updated in the same PR.

A Rust core never talks to a v1 panel. A plugin root whose binary is stale falls back to Python wholesale (the contract probe), and Python stays a coherent legacy threads view (§8, R3).

**Graph (wide): depth-only columns.**
- Columns: `depth 0 (roots) | depth 1 | depth 2 | …`, as deep as the selected root's tree. Labels are `Depth n`, matching V11.
  - `tier` returns `(root_w, agent_w)`.
  - `col_label` (`layout.rs:63-69`) and `Columns::place`'s per-index gaps become uniform.
  - `wide.rs:32-36` becomes `[vec![rw], vec![dw; depth]]`.
- **One card component at every depth.** It merges `graph/orchs.rs`'s orchestrator card and `graph/agent_card.rs`.
  - **Line 1:** harness mark, **title**, the subtree's need marks (✗ ! ?N), and its own state.
  - **Line 2:** activity, then `model · age`. A root shows `tab N · age` instead.
  - Expanded: the existing details, with `role · id · model` moved to an `agent` line.
- `thread_card.rs`, `threads.rs` and `cards/phases.rs` are deleted. So are `cards/pods.rs`'s `top_only` and the done/bg pseudo-cards.
- **Background tasks.** A root's own work (`bg`) becomes a line in the root card's details. The `BG_CARD` card goes.
- **Navigation.**
  - `View.thread`, `done_open` and `remember_thread` go.
  - `focus` becomes the depth column.
  - `Target` loses `Thread` and `Done`. `Fin` and `AgentRef` drop `thread`, so `AgentRef{pane, agent}` is unique per root.
  - `nav.rs` loses `split_threads` and `find_thread`. `tree_entries`, `path_ids` and `parent_key` take a `&Root`.
  - Keys keep their meaning: `↑↓` within a column, `→` into children, `←` to the parent. From a root, `→` goes to its depth-1 child on the remembered path.
- The sideways scrolling, breadcrumbs (`sideways.rs`), click guard and wheel rules are unchanged. Breadcrumbs are made of titles.

**Dock (46 columns): root card plus two agent layers, deeper agents as glyph runs.** This mirrors V11's sidebar.
- **Entry line 1:** unchanged. Tab number, state tile, root **title**, `?N`, state word.
- **Entry line 2:** one tile per **active direct subagent**, coloured by its subtree's most urgent state. Then `✓n` for finished direct children, then `N active · M agents`. Today's `thread_tiles` (`dock/tiles.rs:59`) is retired.
- **The docked entry's list uses one row component for every agent:** own glyph, title, then a right-hand run.
  - **Layer 1** is the root's direct children. Its run shows only `+N✓`, its finished children, because its live children are listed under it.
  - **Layer 2** is their children. Its run shows **all** descendants' states, most urgent first, up to 6, then `+N✓`.
  - Nothing below layer 2 gets a row.
  - Finished siblings fold into one `N finished · names` row per parent.
  - Tree prefixes come from today's `tree_rows.rs`, limited to two levels. `Item::Thread` is replaced by the uniform agent row (`dock/items.rs:19-33`).
- `dock/state.rs` loses `active`/`done` threads and its Codex session-thread filter (`:45-64`).

**Narrow (< 120 columns).**
- The selected root opens its direct children as cards, each with a one-line pod run of its descendants.
- The selected agent shows details. `→`/`←` walk depth as today.
- `narrow/card.rs`'s thread cards and phase lists go.

**Goldens.**
- These compare against JS design references built around threads, and need new references:
  - `crates/panorama-core/tests/{design_goldens,design_deep,design_wide,design_states,design_unseen,dock_layout,graph_columns,wide_layout,narrow_layout,card_title,codex_default_tree,harness_views}.rs`;
  - `tests/fixtures/screens/*.json`;
  - `tests/harness_goldens/*`.
- There is no TUI V11 design yet. See §8, D5.

---

## 5. App mod changes (V11 alignment)

Ownership and process follow COMMON:
- A owns `docs/protocol/**`, `tests/fixtures/app-feed*`, `src/**` and the Rust tests;
- B owns `app/**`, `scripts/app-*` and the Node tests;
- both work in per-slice worktrees from `origin/app/herdr-app-mod`;
- the independent review cycle applies, and root merges.

Shared code (`app/shared/`, regenerated into every mod by `scripts/app-build`):
- **`contract.js`**
  - v2 field tables; `feed.v !== 2` rejects;
  - drop `threadKey` and the thread and phase checks;
  - add a per-root agent-id uniqueness check;
  - accept a dangling or cyclic `parent` as a root child.
- **`tree.js`**
  - `aggregate(root, own)` walks `root.agents`. Drop the empty-failed rule.
  - `forest(root.agents)` builds the whole tree.
  - `rows()` emits the V11 dock rows: layer 1, layer 2 with descendant runs, and per-parent finished folds.
  - `threadMark` is removed.
- **`model.js`**
  - The root `name` follows §2.2's app rule; see D3 for `space_name`.
  - Synthetic roots get `agents: []`.
  - `rootSignature` hashes `agents`.
  - `harnessOf(agent, root)` walks parents in `root.agents`.
- **`peek-caller.js` and `routing.js`**: the scope becomes `{root, agent}`, with no `thread` prop.
- **`marks.js`**: the `phases()` bar renderer goes.
- **`state.js`** (V10 vocabulary, new in `0adfb5f`): unchanged.

Per mod:
- **`panorama` (sidebar), `main.js`**
  - Child count = live direct agents + native children (`:65, 145`).
  - Drop the summary/thread rows (`:85-92`) in favour of the uniform agent row.
  - Rows show `agent.title` instead of `role` (`:96`).
  - Line-2 tiles become direct-subagent tiles.
- **`panorama-graph`**
  - `graph.js:100-114`: delete `threadNodes`. Column 0 holds the space's roots, then depth columns.
    - The existing `nativeLayout` / `nativeFallback` path is the model for this: native terminal children and transcript agents merge into one tree under a root, with the existing no-duplicate rule for feed-root versus native-child.
  - `main.js:22`: state `{root, agent}`. `:192-195`: breadcrumbs are titles; labels are `Depth n`.
  - `cards.js:46-56`: thread card deleted. `:58-63`: one card for root and agent, per V11 line 1/line 2.
  - `actions.js:15-18`: no thread in transcript props.
- **`panorama-peek`**
  - `subject.js:12-16` resolves `{root, agent}`.
  - `main.js:122-123`: title = `agent.title`, falling back to the root title. Subtitle = `role · model`.
  - `:132`: "Open orchestrator" becomes "Open root pane", or "Focus pane".
- **`panorama-pane`**: `space.js:9-11` and `main.js:63` read "N roots · N agents".
- **`panorama-spaces` and `panorama-tabs`**: no direct change. They pick up `aggregate` through regeneration.

Node tests to rewrite:
- `sidebar.test.mjs`: thread mutations, rows, the tasks fold, empty failed thread, healthy thread mark.
- `b3-graph.test.mjs`: the Threads column, "never invents a thread".
- `b4-scope.test.mjs`, `b3-actions.test.mjs:20`, `b3-spaces.test.mjs:8, 18` and `fixtures.test.mjs:10, 23`.
- `build.test.mjs` passes once copies are regenerated.

Under the operator's faster-delivery policy (COMMON lines 68-71), existing tests are **rewritten to the new model**. New coverage beyond that goes to a GitHub issue.

---

## 6. What is removed, and the deprecation path

| Item | Release N (this effort) | Release N+1 |
|---|---|---|
| SessionStart instruction (`src/hook/session.rs:19, 84`; Python `hooks.py:44`) | **Removed at once.** The hook still writes its `session` record; only `additionalContext` stops. | — |
| `report_thread` MCP tool (`src/mcp/**`, Python `mcp_server.py`) | Still listed, with the same schema, so sessions already told to call it do not see "Unknown tool". Its description says "Deprecated: has no effect". A call returns `{status:"ignored", reason:"threads were removed"}` and writes nothing. `server.rs:22` instructions are emptied. | Tool and server removed. The `mcpServers` entry leaves `.claude-plugin/plugin.json`. `scripts/mcp` exits 0 with a one-line note. |
| `panorama emit` (`src/emit/**`, `main.rs:33`) | Accepts the old flags, prints `panorama emit: threads were removed; nothing recorded` to stderr, exits 0, writes nothing. Scripts that call it keep working. | Removed (unknown subcommand). |
| Event channel reading (`collect/events.rs`, `threads/declared.rs`, `collect/codex/{channels,declared}.rs`) | Deleted, nothing reads the files. Existing `…/herdr-panorama/events/*.jsonl` files are **left in place** (no deletion authority); the README says they can be removed. | — |
| `PANORAMA_EVENTS_DIR` | Ignored, documented as obsolete. | Gone from the docs. |
| `done_window` / `PANORAMA_DONE_WINDOW` | **Kept.** The meaning becomes "finished agents" (§2.4). | — |
| Contract `claude` (`src/contract.rs:22`, `hooks/run-hook.sh`, `scripts/mcp`) | Raised to `claude 2`, because the tool text, server instructions and events format changed. A stale binary falls back to the Python hooks and server. Those get the same two minimal changes (instruction removed, `report_thread` a no-op) as a frozen-Python bug-fix, so the fallback path does not keep asking. | — |
| Codex `[mcp_servers.herdr_panorama]` (`codex/config.example.toml:17-22`) | The example is removed. Users who added it keep a working `panorama mcp` (tool listed as a no-op) for the whole of N. `docs/codex.md` says to delete the block. **We never edit `~/.codex/config.toml`** (COMMON authority). | `panorama mcp` without tools, then removed. |
| Plugin descriptions (`plugin.json`, `marketplace.json`, `herdr-plugin.toml:5`), README `:5, 179-181, 280-281, 296-299, 315, 347`, `docs/DESIGN.md:398-480, 571-605, 862-922` | Rewritten to the agent tree. DESIGN's Threads section becomes "Agent tree and names", with a short "Removed: threads" history note. | — |
| `snapshot --json` / `--counts` keys (`orchestrators`, `threads`, `active_threads`) | They change with the model. Documented as a breaking CLI output change in the release notes. | — |

**For Claude plugin users:**
- A plugin update brings the new hooks and the no-op tool together.
- Running sessions that already received the instruction may call `report_thread` for the rest of their life. They get `ignored` back, and nothing breaks.
- Users who never installed the plugin see no difference beyond the new view.

---

## 7. PR-sized slices, in dependency order

Tiers follow COMMON's usage: Low, Medium, High, Safety-critical. Safety-critical means it touches the runtime, installer or personal state. Each slice:
- is one PR into `app/herdr-app-mod`;
- runs the full Rust gates of COMMON line 18, and the Node and `app-check` gates for `app/**`;
- keeps modules under 300 non-test lines;
- updates the existing tests it breaks, and adds no new harness.

**PR #152 (integration → main) stays held for the operator's installed visual approval.** The recommendation (D1) is that this effort lands on the integration branch **before** that approval, so the operator approves V11 as installed rather than approving v2 and then changing it.

| # | Slice | Owner | Depends on | Acceptance | Tier |
|---|---|---|---|---|---|
| U0 | This plan, plus the V11 design commit (operator-owned, separate) | root | — | Operator answers D1–D8 | Low |
| U1a | **Titles, Claude + TUI display (additive).** Add `Agent.title`/`title_source` and `Orchestrator.title`/`title_source`. Implement the Claude precedence of §2.2, including the operator-named tab rule against `neutral_label` and the first-prompt fallback (kit accessor or first-record read). TUI agent cards, dock rows and narrow rows show `title`; role/id/model move to the secondary line. Panel protocol 1.4 (additive), feed v1 additive (unknown fields are ignored). | A | — | Unit tests for every precedence rung on captured fixtures. Existing goldens updated for the title change only. `snapshot --json` shows titles. | Medium |
| U1b | **Titles, Codex.** Root: tab or pane name, then `thread.name`, then first prompt. Subagents: spawn task name (after capturing a real `collabAgentToolCall` and pinning it as a fixture), then `name`, then `agentNickname`, then first prompt. Fixtures with `agentNickname`/`agentRole`. | A | U1a | Captured-fixture tests. `codex_default_tree.rs` updated. No change to discovery budgets. | Medium |
| U2 | **Report channel deprecation** (§6, column N). Instruction removed. `report_thread` and `emit` become no-ops. `claude 2` contract. Python twins changed minimally. Manifests, README and codex docs updated. | A | — (parallel with U1) | `mcp_goldens.rs`, `test_report_thread.py`, `claude_manifests.rs`, `codex_mcp.rs` and the `hook` tests rewritten for the no-op. A hook payload golden shows no `additionalContext`. | Medium |
| U3 | **Collector: uniform tree behind a single-thread adapter.** Collectors build `root.agents` directly. Deleted: thread sources, phases (Claude heuristic and Workflow, Codex turns), `main_card`, `flavour`, the Codex session/declared threads and the `main:codex` root agent. Root own-state moves onto the root. Per-agent window including failed, plus `hidden_finished`. **On the wires, every root is emitted with exactly one synthetic thread** (`id: "agents"`, source `session`, no phases, all agents), so the TUI, panels and v1 app keep working unchanged. Model goldens regenerated from Rust; Python parity retired for them (R3). | A | U1a, U1b | `collect_*`, `codex_*` and `model_goldens` rewritten. No agent lost relative to the old output (a fixture-wide id-set equality test over the existing scenarios). The done-window cases (done, failed, ancestor kept) covered by rewriting the existing window tests. | High |
| U4 | **TUI graph: depth-only columns and the uniform card** (§4). | A | U3 | `graph_columns`, `wide_layout` and `design_deep` goldens re-baselined to reviewed Rust renders (D5). Keyboard and click tests in `src/app/driver/tests/*` updated. Operator screenshot check at 160 and 240 columns. | High |
| U5 | **TUI dock and narrow** (§4). | A | U3 (parallel with U4) | `dock_layout`, `dock_screens`, `narrow_*` and `design_states` re-baselined and reviewed. Operator screenshot check of the 46-column panel. | High |
| U6 | **Model cleanup and panel protocol 2.0.** Delete `Thread`/`Phase`/codes and the adapter. Rename `Orchestrator` → `Root` and `orchestrators` → `roots`. Raise `PANEL`/`TUI` contracts to 2, with the scripts. Update `panel-v1.md` (→ `panel-v2.md`), `snapshot` JSON and counts. | A | U4, U5 | `core_server*`, `proto_client` and `panel_*` tests updated. `scripts/tui` and `scripts/panel` probes tested with old and new binaries (existing tests). | High |
| U7 | **Feed v2 contract and writer** (§3). `app-feed-v2.md`, v2 fixtures, the `lifecycle.json` digests, serializer, probe string `panorama app-feed v2`, the runtime doc wording. | A | U6 | `app_feed_contract`, `app_feed_serialization`, `app_feed_runtime` and `app_feed_atomic` pass on v2. The writer refuses nothing that v2 allows. **Not installed alone.** | Safety-critical |
| U8a | **App shared code, sidebar and Peek** (§5) on the v2 fixtures. | B | U7 merged | Node tests rewritten. `scripts/app-build --check` and `app-check` per mod. Seeded screenshots compared with V11 frames B and C. | Safety-critical (feed-client probe) |
| U8b | **App graph** (§5) on the v2 fixtures. | B | U8a | `b3-graph`/`b3-actions` rewritten. Seeded screenshots compared with V11 frames D and D2, including horizontal scroll and the click guard. | High |
| U9 | **Docs**: DESIGN, README, `app/README.md`, `RECIPE.md`, `docs/codex.md`, `herdr-app-mod.md` amendment (thread wording in its sections 1–4), `rust-rewrite.md` note that the model diverges from Python here. | C | U8b | Docs-only review. No leftover "thread"/"orchestrator" in user docs, except the history note and Codex's own "thread" (R6). | Low |
| U10 | **Integrated install and visual approval.** Integration head built. Uninstall and reinstall of the live app home through the reviewed installer, under `runtime.lock` (same authority as the earlier live install). Retention exceptions verified. The panorama home view (and with it the core) is restarted after the plugin's binary moves to the U6+ build, so no pre-2.0 core refuses the new panels (`docs/protocol/panel-v2.md`, "Version 1"). Operator installed-visual approval, then PR #152 is re-frozen. | root | U9 | Live `errors[]` empty. Layout unchanged. The operator explicitly approves. | Safety-critical |
| U11 | **Release N+1 removals** (§6, column N+1). | A | a later release | Unknown-command and missing-tool behaviour as documented. | Medium |

Ordering notes:
- **U1a, U1b and U2 can run in parallel.** They are user-visible on their own (names appear, the instruction stops) and carry no format risk.
- **U3's adapter keeps every wire stable.** U4 and U5 can then port the TUI against the new tree while viewers still parse 1.x. U6 then breaks the panel wire in one place.
- **U7 and U8a must merge back to back.** Between them, a build of the integration branch has a v2 writer and v1 mods. That is only a problem if installed, and the plan installs only at U10.

---

## 8. Risks and open decisions

### Decisions for the operator (each with a recommendation)

- **D1: when to land, relative to the main visual hold (PR #152).**
  - Option (a): land U1–U9 on `app/herdr-app-mod` and approve V11 installed.
  - Option (b): approve and merge the current v2 design first, then migrate.
  - **Recommendation: (a).** V11 changes the sidebar and graph structure, so approving the old structure first costs a second installed review for a design that is already superseded.
- **D2: Codex subagent title order.**
  - `agentNickname` is assigned by Codex, not written by the spawner. Spawn task arguments are not parsed today, and their field is unverified.
  - **Recommendation:** spawn task name, then `thread.name`, then `agentNickname`, then first prompt. Put the nickname after `name` because it is stable but uninformative. If the capture in U1b shows Codex spawns carry no task text, accept nickname-or-prompt as the Codex subagent name and say so in `docs/codex.md`.
- **D3: `space_name` in the app.**
  - `model.js:23` lets `agent.space_name` win over the tab name and every title.
  - **Recommendation:** an operator-named pane or tab wins, then the session title. Show the space name only as secondary context (V11 already dropped it from line 2 in V10).
- **D4: failed agents and the window.**
  - **Recommendation:** failed agents age out after `done_window`, like today's failed threads. The alternative, keeping failures until seen (the unseen mark), would make the window depend on per-viewer state.
- **D5: TUI design reference.**
  - V11 covers the app only. The TUI goldens compare against design JS that encodes threads.
  - **Recommendation:** no new TUI design turn. Re-baseline the goldens to reviewed Rust renders, and attach screenshots to U4 and U5 for the operator. A TUI V11 port of `swarm-deep.js`/`swarm-focus.js` is optional and would only be needed if the operator wants cell-exact references again.
- **D6: the multi-agent-tab label rule.**
  - **Recommendation:** a tab label names a root only when the tab holds exactly one agent pane. Otherwise use the pane name, else the session title. This drops today's `label · pane` suffix.
- **D7: Python twins.**
  - **Recommendation:** make only the two deprecation edits (instruction off, `report_thread` no-op) in the frozen Python. Leave the Python view showing threads as the legacy fallback, and retire it with the Rust port's end (rust-rewrite D2). The alternative is to remove the Python fallback now.
- **D8: Workflow runs.**
  - A native Workflow run loses its grouping card. Its agents become direct children of the root, titled by description, with the workflow label as `role`.
  - **Recommendation:** accept this, consistent with "no special components". Adding a synthetic "run" node would reintroduce a thread by another name.

### Risks

- **R1: Claude first-prompt fallback needs a kit change.** The kit exposes spawn `description` but not the prompt text. Mitigation: read the first user record through the existing scanner, or add a kit accessor in a kit release. It is a fallback only, because `description` is required by Claude Code.
- **R2: the TUI tab-label heuristic** may misjudge an operator who literally named a tab `claude` or `1`. Mitigation: `/rename` in the session still works as the "1" source. The rule is documented.
- **R3: Python parity is broken on purpose.** The model goldens (`tests/model_goldens.rs`), `tests/test_goldens.py` and the screen fixtures captured from Python stop matching. Mitigation: U3 regenerates them from Rust and records the divergence in `rust-rewrite.md`. The Python fallback stays internally consistent (its own core speaks panel 1.x).
- **R4: the Codex read cap** (8 per refresh) still makes deep trees fill in over several refreshes. Unchanged, but more visible now that depth is the primary axis. The plan does not raise the budget.
- **R5: failed agents never aging out** (`Failed` is live) is a silent regression if U3 forgets D4. U3's acceptance names it.
- **R6: the word "thread" collides with Codex's own term** for a session (`thread/read`, `parentThreadId`, `codex identity`). The cleanup must not rename Codex protocol terms. The U9 grep allowlists `src/codex/**` and `docs/codex.md`.
- **R7: one PR (U3) changes every view's data at once** behind the adapter. Mitigation: the fixture-wide agent-id equality test, plus no install until U10.
- **R8: duplicate identities in the app.** A feed root can also appear as a native terminal child. The existing no-duplicate join rule (COMMON "Stock parity clarification") must carry over to the merged tree in U8b.
