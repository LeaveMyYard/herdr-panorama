# The panorama core ↔ panel protocol, v1

This protocol is for per-tab panels (docs/DESIGN.md, "Per-tab panels"). The Python
implementation is `src/herdr_panorama/protocol.py` (wire format), `core.py` (the core) and
`panel.py` (the viewer). It carries render-ready state rather than Python objects, so a viewer
in another language, such as Rust or ratatui, can speak it.

## Transport

- **Socket.** A Unix stream socket.
  - Location: `$HERDR_PLUGIN_STATE_DIR/core-<session>.sock`, or
    `$XDG_RUNTIME_DIR/herdr-panorama-<uid>/core-<session>.sock` when that path is too long for
    a Unix socket.
  - `<session>` is the first 12 hex characters of the SHA-1 of `$HERDR_SOCKET_PATH`.
  - The socket has mode `0600` and its directory `0700`. The core accepts only peers with its
    own uid.
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

- `{"hello": {"protocol": "1.0", "role": "panel", "pane_id": "<herdr pane>", "tab_id": "<herdr tab>"}}`.
  This is the first message. `pane_id` is a claim: the core checks it against herdr before
  trusting it, and never uses it as authority to close anything.
- `{"seen": ["<agent id>", ...]}`: the operator looked at these agents. A viewer queues these
  while disconnected and sends them again after the next `welcome`.
- `{"bye": {}}`: the viewer is closing.

Any message before `hello` closes the connection.

### Core → viewer

- `{"welcome": {"protocol": "1.0"}}`: the answer to a compatible `hello`. The last `state`
  follows at once, if there is one.
- `{"error": {"message": "protocol", "protocol": "1.0"}}`: an incompatible `hello`. The
  connection then closes.
- `{"state": {...}}`: always a full snapshot (no deltas in v1). It is sent after every collect
  (~1.5 s), on focus and tab changes, and right after `welcome`.
- `{"ping": n}`: every 5 s.

### `state`

| field | meaning |
|---|---|
| `protocol` | `"1.0"` |
| `swarm.generated_at`, `swarm.herdr_ok`, `swarm.inbox_ok`, `swarm.errors`, `swarm.stale_after` | as `model.Swarm` |
| `swarm.orchestrators[]` | `model.Orchestrator` fields, with `threads[]` (`model.Thread`, with `phases[]` and `agents[]`) and `bg[]`, field for field. `model.py` documents each field. |
| `unseen` | `{agent id: "fail" \| "wait" \| "done"}`, the unseen marks |
| `focused_pane` | the pane herdr last reported focused, or `null` |
| `visible_tabs` | the tabs herdr shows, or `null` if unknown. A viewer whose tab is not listed stops animating; unknown counts as visible. |

The viewer picks its own tab's orchestrator (`tab_id` equals its own) as the panel's docked
entry, and renders the docked panel (designs 9e and turn 10).

## Delivery

Each viewer has a bounded outbox of 4 messages. When it is full, the oldest message is
dropped. A viewer that reads slowly therefore loses older states, and never stalls the core.
