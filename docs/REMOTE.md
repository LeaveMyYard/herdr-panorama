# Agents on other machines

herdr 0.9.3 can save SSH machines (`herdr machine list`). It also forwards API calls to a
machine's own herdr server with `herdr --machine <label-or-id> <api command>`; forwarding
never starts a server, and ids are per server. Panorama uses these to show the Claude
orchestrators running on those machines next to the local ones, in the same view.

## What was measured (read-only, against a saved VM)

- **Profiles:** `herdr machine list --json` returns `{id, label, target, session, enabled,
  selected}` for each saved machine. `herdr machine status --json` returns
  `reachable`/error without prompting.
- **Remote snapshot:** `herdr --machine <id> api snapshot` returns the same snapshot shape
  as locally (`agents`, `panes`, `tabs`, `workspaces`, `focused_*`), in about 0.3 s.
- **ssh:** the profile's `target` works with `ssh -o BatchMode=yes` (no prompt), and the
  remote `~/.claude/projects` holds the same transcript layout as locally.
- **Mirroring:** `rsync` over that ssh mirrored one 850 KB transcript in 0.6 s, and 38 ms
  the next time (append-only).
- **Switching machines:** herdr's API has **no call that switches this herdr TUI to another
  machine**. Machine selection exists only in herdr's own UI; the schema's `client.*`
  methods set the window title and shell surface. `herdr --machine m agent focus <pane>`
  changes that machine's server focus, not this TUI.

## Design

- **Which machines.** herdr's enabled saved machines, filtered by `machines` in the
  plugin's `config.toml`: `"all"` (default), `"none"`, or a list of labels such as
  `["devbox"]`. A change applies without a restart: the setting is re-read when the config
  file's mtime changes. Machines that stay selected keep their watcher; only removed
  machines stop and only added ones start.
- **Per machine, a background thread** (`machines.Watcher`) every 5 s. A slow or
  unreachable machine never blocks the local view.
  1. Read the remote herdr state with `herdr --machine <id> api snapshot` (8 s timeout,
     read-only).
  2. Mirror exactly the files the scanner reads of the sessions that snapshot names, into
     the plugin state dir under `remote/<machine id>/projects/`:
     - **`.jsonl`**, append-only: the session transcript, `subagents/agent-*.jsonl`, and the
       workflow `journal.jsonl` and agent transcripts. These go with
       `rsync --append-verify`, so only new bytes travel. An rsync without it (macOS ships
       openrsync) gets a normal pass, whose delta transfer also sends little more than the
       new bytes.
     - **`.json`**, rewritten in place, possibly shorter or at the same size: the agent
       `*.meta.json` files and `workflows/<run>.json`. These go in a second, normal rsync
       pass using size and mtime.
     - Nothing else (`tool-results/`, notes), and no file over 64 MB (`--max-size`).
     - Over `ssh -o BatchMode=yes -o ConnectTimeout=5`, with a shared control connection
       (`ControlMaster=auto`, `ControlPersist=60`) and a 30 s timeout per pass. rsync runs
       in its own process group, and a timeout kills the whole group, ssh included.
     - A saved ssh target that starts with `-` or contains whitespace is never used.
     - **Pruning:** `seen.json` in the mirror records when each session was last named.
       A session not named for longer than the done window has its mirrored files
       deleted, so the mirror stays bounded.
     - **Only well-formed session ids**, UUIDs, ever become rsync rules or paths to
       delete. A malformed one is dropped: empty, `..`, or anything with a separator.
       Pruning checks this twice: `seen.json` keeps only UUID keys, and a file name must be
       a UUID before anything is removed. It skips symlinks, and deletes a session dir only
       if it resolves to a real directory directly inside its project dir.
     - **One sync per machine at a time:** a lock per machine around mirror + prune, so
       two refreshes can never append into one mirror at once.
  3. Run the same collector as for local sessions, with that mirror as its projects root.
     Every orchestrator then gets its machine (`machine_id` for identity and commands,
     `machine` the label for display). Its herdr ids are namespaced by the machine's
     **id** (`<id>:w1:p2`), because ids are per server and labels can repeat or contain `:`.
- **Merging.** The view adds every machine's latest result after the local orchestrators,
  one machine after another. Problems show in the header's `refresh failed` chip:
  - **unreachable:** a machine that has never answered, with its error;
  - **stale:** a machine whose last good result is older than 30 s, or whose last refresh
    failed; it keeps that last result on screen.
- **Identity.** Cursors, clicks and arming compare namespaced ids, so `w1:p1` here and
  `devbox:w1:p1` there are different orchestrators.
- **UI, the operator's choice: group by machine.** A faint divider row with the machine's
  label sits above that machine's orchestrators: `─ devbox ───`. Groups are keyed by the
  machine's id, so two machines with the same label get two dividers. The cards and entries
  themselves are unchanged, and local orchestrators have no divider. This applies to the
  2a wide and narrow layouts and to the docked 3a panel. The divider is not a click target.
- **Enter, `g` or a click on a remote orchestrator, the operator's choice: focus remotely
  plus a hint.** Panorama runs `herdr --machine <id> agent focus <pane>` in a worker,
  so the pane is in front once herdr shows that machine. It then notifies
  "<name> is on <machine>: switch herdr to <machine> to see it". It **never docks** into a
  remote server and never focuses anything locally; the cockpit and follow are local
  only.
- **The inbox:** remote agents get no `?N` and `a` says "not supported yet". Answering them
  waits for herdr-inbox's remote support.
- **New agent:** stays local.

## Limits of this version

- Panorama's own Claude Code plugin data (the explicit `report_thread` channel, the
  SubagentStart/Stop hook times) is not mirrored. Remote agents therefore show inferred
  endings (`done?`), and declared threads are missing until those files are mirrored too.
- A transcript rewritten shorter on the remote keeps its old mirrored copy until it grows
  past it; transcripts are append-only in practice.
- `CLAUDE_CONFIG_DIR` on the remote is not honoured: the mirror reads `~/.claude/projects`.

Background work on another machine (design turn 8, "waiting") is judged from its transcripts
alone: its processes are not checked, so an open background task counts for at most 24 hours
after it started (`BG_CAP`).
