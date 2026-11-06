# Agent resume (optional, a stopgap)

**This exists only until herdr-app or herdr-core resumes agents itself.** Upstream has tied
that to OSC 7501 reporting a resume reference ([herdr-core#13](https://github.com/herdrdev/herdr-core/issues/13);
also herdr-app#99 and #406). When that lands, delete `src/app_feed/resume/`, its toggle, its
report, its rows and this page. Keep tolerating what it left behind, or anyone who ever turned it
on gets refused: `resume.json` and `.resume.tmp` in `receipt.rs` and `runtime.py`, and
`agents.resume` and `agents.resume.clicks` in `preferences.py`.

After a full core stop, crash or reboot, kept terminals come back as plain shells, at the same
address and folder. This feature lists what was lost, offers a **Resume** button on each pane, and
after a reboot can type the agent's resume command by itself, **with nobody at the machine**.

How often it types by itself depends on the layout the app opens in. herdr-app restores the last
layout, and falls back to `main` when a mod that layout needs is missing. The Panorama layout
references `agent-usage`, which Panorama doesn't install.

| Install | Opens in | After a reboot |
|---|---|---|
| With `agent-usage` | `panorama`, after quit, SIGTERM or SIGKILL | **Usually types by itself**: 4 of 4 with nobody acting, writer up about 0.5 s after app start |
| Without it | `main`, even after a clean close | Nothing runs until someone switches, so usually a button |

Automatic typing needs the writer to see the shell come back. Measured with the real app on a
simulated reboot, the shell came back about 2.6 s after app start, and the sidebar ran `--ensure`
within 0.1 s of the layout showing:

| Layout shown after app start | Result |
|---|---|
| 0.4 s, 1.5 s | typed by itself, in all three panes |
| 3 s | a button on all three panes |

A later arrival has no other proof of an empty line, so the button is the honest result.

## What is covered

A session is covered only if Panorama was shown at some point in that app run. **The toggle says
"on" either way**, and in `main` nothing shows that a session isn't covered.

| Situation | Recorded? |
|---|---|
| App opened in `main`, Panorama never shown, agent started | **No.** No writer, no record, and no row after a loss |
| Panorama shown once, then back to `main`, agent started | **Yes.** The writer keeps running |
| Closed and reopened within 30 s, still in `main` | **Yes.** The same writer carries on |
| Closed longer than 30 s, reopened in `main` | **No**, for new agents. Earlier ones stay listed with `closed` and get a button once Panorama is shown |

It is **off by default**. With it off, the feed writer makes no extra `hc` call and types nothing.
Each collect it lists `data/`, reads the panorama-spaces store and checks for a record to remove,
and its file watch includes that store's folder.

## Toggle

**Resume agents** in the Agents header's menu (the clock or layers icon, below Sort agents).
The choice is `agents.resume` in the retained `data/panorama-spaces/store.json`, beside
`agents.scope`: `"on"`, or absent for off. The writer reads it at every collect and again right
before typing (herdr#4894). That last read is not atomic with the send, so turning it off in that
instant can still type. An unreadable store neither types nor clears anything.

## What is recorded

`data/panorama/runtime/<install_id>/resume.json` (mode 0600), staged in `.resume.tmp`, synced and
renamed over the last save, written on every change while the toggle is on:

- per terminal: address, harness, session id, folder (the agent's own `foreground.cwd`), the
  terminal's `pid` and `created_ms`, the agent's pid, how it was launched, and whether it used
  the default Claude config dir;
- the core's permanent id, its holders' instance ids, and the kernel's boot id;
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

## A deliberate stop

`hc server stop` and a crash look the same afterwards: a new core reports `stopped_by_restart`
either way. So a deliberate stop can't be proved, only a reboot can: the saved and current
`/proc/sys/kernel/random/boot_id` differ. A missing id on either side proves nothing, and a failed
read never overwrites a saved id. A loss without two different ids, or on a system with no boot id
such as macOS, is never typed by itself. On Linux, then, only a reboot can type by itself.

Every `hc` command starts a stopped core, so with the app open the writer's polling brings the
core back within seconds. That predates this feature. The terminals return as shells, and nothing
is typed until someone clicks.

## Was the session live when the core was lost?

The writer stops when the app is closed, so a record can be old. `closed` says so:

- **The writer was killed with everything else** (a reboot with the app open): `closed` is false,
  and the record is as fresh as the last collect.
- **The writer left on purpose** (30 s after the app closed, or on uninstall): `closed` is true.
  From then on nothing watched, so the operator may have quit the session. A loss after this gets
  a button, not typing.

When a new writer starts and finds no loss, every record must be proved again before it counts.

## When it types

All of these must hold. Any that can't be proved means no typing.

Decided when the loss is seen (failing one means a button, not typing):

1. A proved loss with a new boot id.
2. The record was proved at the last look, its agent was still there, and it was not `closed`.
3. The launch is on the allow-list: `claude` alone or with `--resume <id>`, `-r <id>`,
   `--session-id <id>`, `--continue`, `-c`; `codex` alone or with `resume [<id>]`. `--model`,
   `--effort`, `--verbose` and `--no-daemon` are allowed too. Everything else is shown, not typed.
   An npm-style `node /path/claude` has its script as the first argument, so it's button-only.
   Also shown, not typed: `--remote-control`, `rc` (herdr#5130), `-p`, `--print`, `--bg`, `--tmux`,
   `--permission-mode`, a prompt argument and anything unknown.
4. Claude ran under the default config dir, which is what a new shell gets. Codex's isn't checked.
5. The core has been up at most 5 minutes.

Read from the terminal. A program in front of the shell **before** any prompt was seen may be an
rc file's child, so Panorama keeps watching and ends with a button. After a prompt it's a final
skip with a button. A terminal that finished, or isn't a known shell, ends with no button, since
nothing could make it work.

6. Same address and `created_ms`; live, kept, with no `reason`.
7. No status report; the foreground process is the terminal's own, with the same argv and a name
   of `bash`, `zsh` or `fish`. `exec` keeps the pid, so the argv is what catches `exec ssh box`.
   These three shells were tested. No other shell is ever typed into.
8. Its folder equals the recorded one (herdr#3116). Panorama does not `cd`.

### Proof the line is empty

A shell can't be asked what is on its command line, and "unchanged" proves nothing: a person who
types and pauses looks settled. So Panorama proves the line is empty by witnessing the first screen:

9. It saw the terminal with no shell yet, or a blank screen, and then read the first drawn screen.
   Reads run about every 200 ms while it waits; they run in series and share a thread with
   collects, so that stretches with many terminals. If the shell was already up at first sight,
   it can't prove this, and offers a button.
10. The screen stays **exactly** that first screen for 1.5 s, and is still that at the last look.
    Any change rules out typing by itself for good: someone typed, or used the shell. A prompt
    that redraws itself lands here too. That costs a button, not a wrong line. Panorama keeps
    watching until the screen stops moving, so the button knows the final prompt.

Keys from a viewer that land before the first read are still caught: the settle time is shorter
than the typing gate below, so the last look refuses them. `hc keys` from a program doesn't trip
that gate, and could land unseen in that gap.

### Why not `hc send --if idle`

A plain shell has no status, so `--if idle` always refuses it (`status_mismatch`, "t1 is silent,
not idle"), and nothing can be typed that way.

### The race that remains

**hc can't check and type in one step, so this is reduced, not closed.** The order is: save the
attempt, ask whether a person is typing, read the terminal and screen, read the toggle, send.

- Asking is `terminal.keys` with no keys. It waits out the same gate as `send`, then does nothing,
  so a late answer (500 ms) means someone typed in the last ~3 s, and that's a final skip. It
  also means `send` itself normally won't wait. A `send` can't be cancelled once started.
- With nobody typing, the gap from the last read to the paste is a few milliseconds.
- A person who starts typing inside that gap makes `send` wait for them, for seconds. Text then
  lands wherever the terminal is by then, such as a `cat` they started.
- `foreground` is itself refreshed about twice a second, so a program started in the last 500 ms
  may not show.

A real fix needs an atomic option upstream, such as `--if silent` or "only if foreground pid is
N". Nothing has been filed.

## One attempt

The attempt is saved as `sending`, durably, **before** those last reads. If that save fails,
nothing is typed. A writer that finds `sending` after a restart treats the attempt as spent.
After a send, success or failure, Panorama only reads:

- the proved link appears: **resumed** (with a note if the id changed);
- a program runs but no link appears in 30 s: **resumed**, marked unproved;
- a program started and then exited: **failed**, with the id and command kept (herdr#5532);
- nothing started in 30 s: **failed**, saying whether the text reached the screen.

Nothing is retried automatically. A later loss is a new attempt only for a session proved since.

## Resume button

Skipped and failed rows get a button unless nothing could make it work. It takes two steps, because
only a person can judge a line Panorama can't prove empty.

1. **Resume** types nothing. The writer runs the terminal checks (rules 6 to 8) and the typing
   check, then shows the last three non-blank lines on the row, each cut from the start so the
   end stays, with **Type here**. Its tooltip has more of the screen. If someone is typing, or a
   check fails, the row says why and there is no **Type here**.
   - There is also no **Type here** unless the cursor is known to be on the last line. That line
     must start with the prompt last seen when the shell settled, and must not end in a
     backslash. So a completion listing that appears below that prompt, a `>` continuation or
     fish's blank continuation line refuses. Clear the line and press **Resume** again. A prompt
     that really changed, such as one showing an exit status, refuses too.
   - **The exception:** if a listing or pager was already on screen when the shell settled, it
     counts as the prompt, so **Type here** is offered. The same goes for a row that never saw a
     prompt, which takes the line at the first click as its prompt. See Known limits.
     **If the row shows no prompt, don't press Type here.**
2. **Type here** sends, if the checks still pass and the screen still matches what was shown.
   Digits are ignored in that comparison, so a ticking clock can be confirmed; a digit typed onto
   the line changes its length, so it refuses. Anything else that moves between the clicks refuses,
   and the person looks again.

So the person sees the line, half-typed or not, before Enter is pressed. A click lifts rules 1 to 5,
9 and 10 and nothing else.

A press is `panorama-spaces.agent-resume-press-<terminal>-<loss id>[-<token>]`, stored in
`agents.resume.clicks`. The writer ignores it unless the loss id matches this row, it is under 60 s
old, not stamped ahead of the clock, newer than the loss and not already acted on. The token comes
from the feed, and a view lasts 60 s. It binds a press to a view and isn't a secret. So a stored or stale click can't fire later with nobody there.

This proves something asked, not a person: `herdr-app do` can press these too, and any program of
this user can read the feed and press both. That gives it nothing new: it could type into the
terminal itself, and the text is fixed.

## Where it runs

- **Core lost while the app was closed:** a button. This includes quitting the app and rebooting.
- **App opens first after a reboot:** nothing runs until the Panorama layout is shown. Then it
  waits while there is no holder or shell. If the shell beat the writer, rule 9 can't be met: a
  button. See "What is covered" above.
- **Writer restarts mid-way:** `sending` is spent; the first screen and whether it was witnessed
  are saved; the 1.5 s starts over. Without a reboot it's a button. Otherwise a quit session can
  come back only if the writer alone was killed, the session was quit and the machine rebooted
  before the sidebar restarted the writer, about 5 s. The harm is a closed conversation reopened
  in an idle shell.
- **Two writers:** one app home has `writer.lock`. Two app homes on one core are kept apart by
  an exclusive lock on `$XDG_STATE_HOME/herdr-panorama/resume/<core id>.lock`, held for the
  writer's life. The loser records nothing, types nothing and reports the error. This empty file
  survives uninstall.

## Other terminals

Local terminals only. A remote `machine/tN` has no proved link, so it is never recorded and never
typed into. It does not appear in the result. Neither do terminals whose session was never
proved, such as Codex without its hook or `codex-bind`.

## Result

The feed's `resume` field ([feed v2](app-feed-v2.md)) drives a **Resume** line at the top of the
Agents list: counts, opening to one row per terminal with its outcome, reason and any Resume button,
and the command and any note as a tooltip. Options such as `--model` aren't repeated, and the row says
so. It opens by itself when a row has a button. `×` hides it for this view. A record error is in
the field's `error`. A resumed row can read `sent` for up to 30 s while the new agent's link is proved.

## Known limits

- The race above, and `hc keys` from a program before the first read.
- For a shell first seen after it started, nothing proves which line is the prompt. If a person's
  unfinished completion was on screen when the shell settled, **Type here** is offered and can
  submit a half line the row doesn't show. Measured: a one-row listing in zsh or fish shows the
  half line; a five-row fish listing pushes it off the row, leaving it in the tooltip only; bash's
  `--More--` pager hides it from both, and the half line ran with the resume text appended. It
  takes two deliberate clicks on a view with no prompt, on button rows only, and the worst result
  is that person's own half line submitted. Automatic typing isn't affected.
- The confirm step's comparison ignores digits, so a digit swapped for a digit between the clicks
  passes: shown `sleep 1`, it could submit `sleep 9`. The person had already confirmed a half line.
- A screen and a view are kept in `resume.json` and the feed for up to 60 s, both mode 0600.
- A dialog drawn by the shell itself, stable from its first screen, looks like a prompt. zsh's
  first-run menu is one, for a user with no `.zshrc`.
- The command runs with whatever `claude` or `codex` the shell finds, and enters its history.
- Tested with real bash, zsh and fish and a stand-in agent, not a real Claude or Codex resume.
- A session quit in the last moments before a reboot, less than one collect earlier, may look live.
