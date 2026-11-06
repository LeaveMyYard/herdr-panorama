# Agent resume (optional, a stopgap)

**This exists only until herdr-app or herdr-core resumes agents itself.** Upstream has tied
that to OSC 7501 reporting a resume reference ([herdr-core#13](https://github.com/herdrdev/herdr-core/issues/13);
also herdr-app#99 and #406). When that lands, delete `src/app_feed/resume/`, its toggle, its
report and this page. Nothing else depends on them.

After a full core stop, crash or reboot, kept terminals come back as plain shells, at the same
address and folder. This feature types the agent's resume command into those shells.

It is **off by default**. With it off, the feed writer reads one small file per collect, makes
no extra `hc` call, types nothing, and removes any old record.

## Toggle

**Resume agents** in the Agents header's menu (the clock or layers icon, below Sort agents).
The choice is `agents.resume` in the retained `data/panorama-spaces/store.json`, beside
`agents.scope`: `"on"`, or absent for off. The writer reads it at every collect and again right
before typing (herdr#4894). An unreadable store neither types nor clears anything.

## What is recorded

`data/panorama/runtime/<install_id>/resume.json` (mode 0600), staged in `.resume.tmp`, synced and
renamed over the last save, written on every change while the toggle is on:

- per terminal: address, harness, session id, folder (the agent's own `foreground.cwd`), the
  terminal's `pid` and `created_ms`, the agent's pid, and how it was launched;
- the core's permanent id and its holders' instance ids;
- `closed`, set when the writer leaves on purpose.

The session id is only ever the proved link (session-linking.md §4), never the collector's
continued id. A record ends only on a positive read: the terminal is gone, it finished by
`exited` or `killed`, or the agent has been missing for 4 s with the core still up. That wait
outlasts the core's own 3 s stop sequence, during which `hc` still answers. A failed read, or a
core reporting `server_stop`, `stopped_by_restart`, `holder_lost` or `signaled`, never ends one.

## Telling a loss from a restart

`hc server status` lists the holders, the processes that actually run the programs. Each has a
random `instance` id, and every terminal's program dies with its holder. Measured on a private
core (hc 0.1.18):

| Event | Core pid | Holder instance | Terminal `pid` | Result |
|---|---|---|---|---|
| App restart | same | same | same | no loss |
| `hc server restart` | new | same | same | no loss |
| Core SIGKILL | new | same | same | no loss |
| `hc server stop`, then a new core | new | new | new | loss |
| Reboot (core, holder and programs all killed) | new | new | new | loss |
| A kept program dying by signal | same | same | new | no loss: the agent just ended |

A loss is proved when **none** of the recorded holders is still listed, under the same core id.
An empty holder list means a restarted core has not started its holder yet, so nothing is decided
and the record waits untouched. A different core id means the state was wiped: every address is
void and the record is dropped. No timing is involved. Each terminal must then also show a `pid`
other than the recorded one.

## Was the session live when the core was lost?

The writer stops when the app is closed, so a record can be old. `closed` says so:

- **The writer was killed with everything else** (a reboot with the app open, a crash): `closed`
  is false, and the record is as fresh as the last collect.
- **The writer left on purpose** (30 s after the app closed, or on uninstall): `closed` is true.
  From then on nothing watched, so the operator may have quit the session. A loss after this is
  **shown, not typed**.

When a new writer starts and finds no loss, every record must be proved again before it counts.

## When it types

All of these must hold, read fresh, immediately before typing:

1. A proved loss, and this is the first attempt for this terminal in it.
2. The record was proved at the last look, its agent was still there, and it was not closed.
3. Its launch can be repeated. `claude rc`/`remote-control` cannot (herdr#5130), nor can Codex
   subcommands other than `resume`.
4. The core has been up at most 5 minutes when the loss is seen. Longer, and someone may have
   used the shell already.
5. Same address and same `created_ms`; live, kept, with no `reason`.
6. No status report, and the foreground process is the terminal's own program: a bare shell.
7. Its folder equals the recorded one (herdr#3116). Panorama does not `cd`.
8. The screen is drawn, and two reads at least 1.5 s apart are identical.
9. The toggle is still on and the installation still active.

Any foreign foreground or status is a **final** skip: the person may be using that terminal. No
shell within 2 minutes is a skip. Every give-up ends in not typing. Skipped entries keep the
command, so the session can be resumed by hand.

Typed text is `claude --resume <id>`, `codex resume <id>`, or `codex --no-daemon resume <id>`
when the session ran with `--no-daemon` (hc needs it, or Codex's commands run outside the
terminal). The id must be exactly a UUID, checked again when the command is built.

### Why not `hc send --if idle`

A plain shell has no status, so `--if idle` always refuses it (`status_mismatch`, "t1 is silent,
not idle"), and nothing can be typed that way. Panorama sends without `--if`, after the checks
above. `hc send` itself still refuses a blocked program and a lost terminal, and waits for a
person to stop typing. There is no atomic "only if silent", so a gap of a few milliseconds
remains between the last read and the send.

## One attempt

The attempt is saved as `sending`, durably, **before** the send. If that save fails, nothing is
typed. A writer that finds `sending` after a restart treats the attempt as spent. After a send,
success or failure, Panorama only reads:

- the proved link appears: **resumed** (with a note if the id changed);
- a program runs but no link appears in 30 s: **resumed**, marked unproved;
- a program started and then exited: **failed**, with the id and command kept (herdr#5532);
- nothing started in 30 s: **failed**, saying whether the text reached the screen.

Nothing is ever retried. A later loss is a new attempt only for a session proved since.

## Where it runs

- **Core lost while the app was closed:** `closed` is true, so it is shown, not typed.
- **App opens first after a reboot:** the first look may show no holder, or a lost terminal with
  no shell yet. It waits.
- **Writer restarts mid-way:** `sending` is spent; waiting entries carry on from the saved
  detection time; the settle check starts over.
- **Two writers:** one app home has `writer.lock`. Two app homes on one core are kept apart by
  an exclusive lock on `$XDG_STATE_HOME/herdr-panorama/resume/<core id>.lock`, held for the
  writer's life. The loser records nothing, types nothing and reports the error.

## Other terminals

Local terminals only. A remote `machine/tN` has no proved link, so it is never recorded and never
typed into. It does not appear in the result. Neither do terminals whose session was never
proved, such as Codex without its hook or `codex-bind`.

## Result

The feed's `resume` field ([feed v2](app-feed-v2.md)) drives a **Resume** line at the top of the
Agents list: counts, opening to one row per terminal with its outcome and reason, and the command
as a tooltip. `×` hides it for this view. A record error is in the field's `error`.

## Known limits

- Options the session was started with, such as `--model` or `--settings`, are not repeated.
- Something typed but not submitted before the first screen read is not detected: the screen
  stays still, and the command lands after it.
- A dialog drawn by the shell itself looks like a bare shell.
- A writer killed by itself, with the core left running, leaves a gap that `closed` cannot show,
  until the sidebar restarts it.
- The `ending` window: a session quit less than a collect before a loss may still look live.
