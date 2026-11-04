# The panorama core ↔ panel protocol, v2

This protocol is for per-tab panels (docs/DESIGN.md, "Per-tab panels"). The Rust
implementation is `src/proto/` (the viewer's side and the wire format) and `src/core/` (the
core). It carries render-ready state rather than language objects, so any viewer can speak it.

2.0 changed one thing from 1.x: `state.swarm` is the uniform agent tree
(`docs/plans/uniform-agent-tree.md` §2.1, slice U6). Each root has one flat `agents` list; there
are no threads or phases. Transport, messages and delivery are as in 1.4. The Python
implementation (`src/herdr_panorama/protocol.py`, `core.py`, `panel.py`) still speaks 1.x, its
legacy threads view (plan §8, D7 and R3), with its own core. See "Version 1" below.

## Transport

- **Socket.** A Unix stream socket.
  - Location: `$HERDR_PLUGIN_STATE_DIR/sockets/core-<session>.sock`, or
    `$XDG_RUNTIME_DIR/herdr-panorama-<uid>/core-<session>.sock` when that path is too long for
    a Unix socket.
  - `<session>` is the first 12 hex characters of the SHA-1 of `$HERDR_SOCKET_PATH`.
  - The socket has mode `0600`. Its directory must be a real directory (not a symlink),
    owned by the core's user, with mode `0700`; otherwise the core does not start.
  - The core accepts only peers with its own uid, and a viewer talks only to a socket served
    by its own uid (`SO_PEERCRED` on Linux, `LOCAL_PEERCRED` on macOS).
- **Limits.** A connection must send `hello` within 5 s. The core keeps at most 64 connections
  and refuses more. A send that has not finished after 10 s drops that viewer.
- **One core per herdr session.** The core holds an exclusive `flock` on `core-<session>.lock`,
  next to the socket, for its whole life.
- **Framing.** One JSON object per line, UTF-8, `\n`-terminated.
  - A line longer than 4 MiB closes the connection.
  - A line that is not a JSON object is skipped.

## Versions

`protocol` is `"MAJOR.MINOR"`.
- A peer with another MAJOR is refused, with one exception: an open key (`"role": "key"`)
  saying major 1 is answered. A key connection sends `open`, reads `opened` and gets no
  `state`, and those messages did not change in 2.0. herdr's open action runs the Python
  `panorama open`, which says 1.x, also when the core is the Rust one.
- MINOR grows only with additive fields.
- Unknown fields and unknown messages are ignored.

## Messages

### Viewer → core

- `{"hello": {"protocol": "2.0", "role": "panel", "pane_id": "<herdr pane>", "tab_id": "<herdr tab>"}}`.
  This is the first message. `pane_id` is a claim. The core trusts it only when herdr reports
  that pane's foreground process as the connecting process itself (its pid), running
  `panorama panel`. It never uses it as authority to close anything.
- `{"seen": ["<agent id>", ...]}`: the operator looked at these agents. A viewer queues these
  while disconnected and sends them again after the next `welcome`.
- `{"bye": {}}`: the viewer is closing.
- `{"focusing": {"pane_id": "<herdr pane>"}}` (1.2): a panel is about to switch tabs and put the
  keyboard on that pane (the target tab's panel). The core takes it only from a panel whose
  `hello` was verified, and publishes it at once as `state.focused_pane`, so the target panel
  paints its focused look while still hidden. herdr's own focus event follows.
  - A panel whose switch then fails announces its own pane again.
  - If herdr reports no focus within 0.75 s of an announcement, the core reads herdr's real
    focus and publishes that instead.
- `{"filter": {"name": "all" | "running" | "needs"}}` (1.3): the operator chose this filter in a
  panel. The core's home panorama takes it (an unknown name is ignored) and publishes it as
  `state.filter`, so every panel and the home view show the same filter. The choosing panel
  shows it at once and keeps it against older states until the core echoes it, or for 1 s.
- `{"open": {}}` (1.1): from the open key, which says `hello` with `"role": "key"`. The core
  focuses this tab's panel, makes one, or shows the home tab, and answers with `opened`. A key
  connection gets no `state`.

Any message before `hello` closes the connection.

### Core → viewer

- `{"welcome": {"protocol": "2.0"}}`: the answer to a compatible `hello`. The last `state`
  follows at once, if there is one.
- `{"error": {"message": "protocol", "protocol": "2.0"}}`: an incompatible `hello`. The
  connection then closes.
- `{"state": {...}}`: always a full snapshot (no deltas). It is sent after every collect
  (~1.5 s), on focus and tab changes, and right after `welcome`.
- `{"ping": n}`: every 5 s.
- `{"opened": {"result": "panel" | "home" | "failed" | "busy"}}` (1.1): the answer to `open`.
- `{"shutdown": {}}` (1.1): the panels go (the home panorama quits, or per-tab is turned off).
  The viewer exits, and herdr closes its pane with it. A viewer that has had no core for
  10 minutes exits as well.

### `state`

| field | meaning |
|---|---|
| `protocol` | `"2.0"` |
| `swarm.generated_at`, `swarm.herdr_ok`, `swarm.inbox_ok`, `swarm.errors`, `swarm.stale_after` | as the model's `Swarm` (`crates/panorama-core/src/model/swarm/mod.rs`) |
| `swarm.roots[]` | one per agent session in a herdr pane, the depth-0 node of its tree: the model's `Root` fields, field for field, with `bg[]`. Its own state is on it: `herdr_status`, `pending` (the prompt it waits on), `inbox` and `inbox_texts` (its open questions, including those no agent filed), `bg` (its background work), `last_activity`. |
| `swarm.roots[].agents[]` | every agent the root spawned, at any depth, flat, parents before children: the model's `Agent` fields. `parent` is `null` for a direct child of the root, else the id of another agent of the same root; a dangling or cyclic parent reads as a root child, so no agent is hidden. `depth` is the length of the parent chain (root children are 1). Siblings come most urgent first, then oldest first. |
| `swarm.roots[].hidden_finished` | finished agents (done, failed, cancelled) the done window (`done_window`) left out of `agents`; a viewer shows them as "+N older" in the root's fold. |
| `swarm.roots[].title`, `.title_source`, `…agents[].title`, `.title_source` | what the views call the root or agent, and which rule gave it (plan §2.2): `pane`, `tab`, `session`, `prompt` or `id` for a root, `spawner`, `prompt`, `type` or `id` for an agent. Left out when there is none; a viewer then names a root by `tab_label` and an agent by its role and short id. The source drives no logic. |
| `unseen` | `{agent id: "fail" \| "wait" \| "done"}`, the unseen marks |
| `focused_pane` | the pane herdr last reported focused, or `null` |
| `panels` | (1.1) `{tab id: pane id}` of the per-tab panels: where a switch to that tab puts the keyboard |
| `filter` | (1.3) the filter every view shows: `"all"`, `"running"` or `"needs"`. A viewer moves its cursor only among what it shows; a 1.2 core sends none, and each panel then keeps its own. It is the home panorama's filter, not saved: `all` after a home restart. |
| `visible_tabs` | the tabs herdr shows, or `null` if unknown. A viewer whose tab is not listed stops animating; unknown counts as visible. In v1 this is herdr's one focused tab. herdr 0.9.3 does not say how many clients are attached, or which tab each shows, so with two clients the second client's panel can pause until its tab is focused. |

The viewer picks its own tab's root (`tab_id` equals its own) as the panel's docked entry, and
renders the docked panel (designs 9e and turn 10).

## Delivery

Each viewer has a bounded outbox of 4 messages. When it is full, the oldest message is
dropped. A viewer that reads slowly therefore loses older states, and never stalls the core.

## Version 1 (legacy)

1.0 to 1.4 are what the Python core and panel speak (`src/herdr_panorama/protocol.py`,
`PROTOCOL = "1.3"`), and what a Rust build from before 2.0 spoke (1.4). They differ from 2.0
only in `state.swarm`:
- `swarm.orchestrators[]` instead of `roots[]`, each with `flavour`, `hidden_done` and
  `threads[]` instead of `agents[]` and `hidden_finished`;
- each thread has `id`, `name`, `source`, `phases[]`, `agents[]`, `status`, `started`, `ended`,
  `worktree`, `pct` and `flavour`, and each agent has `phases` (indices into its thread's);
- 1.1 added `panels` and `open`, 1.2 `focusing`, 1.3 `filter`, 1.4 the titles.

A 2.x viewer refuses a 1.x core and the other way round, except for the open key above. The
launchers keep the two apart: `scripts/panel` and `scripts/tui` run the Rust binary only when
it answers contract 2 (`panorama contract`, `panorama contract tui`), so the view and its panels
are both Rust (2.x) or both Python (1.x).
