# The panorama core ↔ panel protocol, v1

This protocol is for per-tab panels (docs/DESIGN.md, "Per-tab panels"). The Python
implementation is `src/herdr_panorama/protocol.py` (wire format), `core.py` (the core) and
`panel.py` (the viewer). It carries render-ready state rather than Python objects, so a viewer
in another language, such as Rust or ratatui, can speak it.

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
- A peer with another MAJOR is refused.
- MINOR grows only with additive fields.
- Unknown fields and unknown messages are ignored.

## Messages

### Viewer → core

- `{"hello": {"protocol": "1.2", "role": "panel", "pane_id": "<herdr pane>", "tab_id": "<herdr tab>"}}`.
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
- `{"open": {}}` (1.1): from the open key, which says `hello` with `"role": "key"`. The core
  focuses this tab's panel, makes one, or shows the home tab, and answers with `opened`. A key
  connection gets no `state`.

Any message before `hello` closes the connection.

### Core → viewer

- `{"welcome": {"protocol": "1.2"}}`: the answer to a compatible `hello`. The last `state`
  follows at once, if there is one.
- `{"error": {"message": "protocol", "protocol": "1.2"}}`: an incompatible `hello`. The
  connection then closes.
- `{"state": {...}}`: always a full snapshot (no deltas in v1). It is sent after every collect
  (~1.5 s), on focus and tab changes, and right after `welcome`.
- `{"ping": n}`: every 5 s.
- `{"opened": {"result": "panel" | "home" | "failed" | "busy"}}` (1.1): the answer to `open`.
- `{"shutdown": {}}` (1.1): the panels go (the home panorama quits, or per-tab is turned off).
  The viewer exits, and herdr closes its pane with it. A viewer that has had no core for
  10 minutes exits as well.

### `state`

| field | meaning |
|---|---|
| `protocol` | `"1.2"` |
| `swarm.generated_at`, `swarm.herdr_ok`, `swarm.inbox_ok`, `swarm.errors`, `swarm.stale_after` | as `model.Swarm` |
| `swarm.orchestrators[]` | `model.Orchestrator` fields, with `threads[]` (`model.Thread`, with `phases[]` and `agents[]`) and `bg[]`, field for field. `model.py` documents each field. |
| `unseen` | `{agent id: "fail" \| "wait" \| "done"}`, the unseen marks |
| `focused_pane` | the pane herdr last reported focused, or `null` |
| `panels` | (1.1) `{tab id: pane id}` of the per-tab panels: where a switch to that tab puts the keyboard |
| `visible_tabs` | the tabs herdr shows, or `null` if unknown. A viewer whose tab is not listed stops animating; unknown counts as visible. In v1 this is herdr's one focused tab. herdr 0.9.3 does not say how many clients are attached, or which tab each shows, so with two clients the second client's panel can pause until its tab is focused. |

The viewer picks its own tab's orchestrator (`tab_id` equals its own) as the panel's docked
entry, and renders the docked panel (designs 9e and turn 10).

## Delivery

Each viewer has a bounded outbox of 4 messages. When it is full, the oldest message is
dropped. A viewer that reads slowly therefore loses older states, and never stalls the core.
