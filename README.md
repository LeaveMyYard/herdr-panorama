<p align="center"><img src="https://img.shields.io/badge/herdr%20panorama-every%20agent%2C%20one%20live%20screen-5865a3?style=flat-square&labelColor=24292f" alt="herdr panorama | every agent, one live screen"></p>

<h3 align="center">Run a swarm of Claude Code agents in herdr and see, at a glance, which one needs you</h3>

<p align="center">herdr panorama draws every Claude Code orchestrator in your <a href="https://herdr.dev">herdr</a> session, the threads of work it drives and the subagents it spawned, like an Argo resource graph with small coloured status tiles. It reads what Claude Code already writes, types into a pane only when you start a new agent from it, and puts what failed, blocked or asked a question in front.</p>

<p align="center"><a href="docs/media/herdr-demo.mp4"><img src="docs/screenshots/hero.gif" width="88%" alt="The swarm view on demo data: orchestrator cards with spinners and blinking attention edges, a PR thread with its steps, and its agents"></a></p>

```sh
herdr plugin install LeaveMyYard/herdr-panorama
```

<p align="center"><sub>✓&nbsp;Free,&nbsp;MIT&nbsp;licensed &nbsp; ✓&nbsp;Reads&nbsp;local&nbsp;transcripts,&nbsp;no&nbsp;hosted&nbsp;service &nbsp; ✓&nbsp;macOS&nbsp;and&nbsp;Linux,&nbsp;herdr&nbsp;0.9+</sub></p>

<p align="center">
<a href="https://herdr.dev"><img alt="herdr" src="https://img.shields.io/badge/herdr-%E2%89%A5%200.9-5865a3?style=flat"></a>
<img alt="Claude Code plugin" src="https://img.shields.io/badge/Claude_Code-plugin-d97757?style=flat&logo=claude&logoColor=white">
<img alt="MCP server" src="https://img.shields.io/badge/MCP-server-1f2937?style=flat&logo=modelcontextprotocol&logoColor=white">
<img alt="Python" src="https://img.shields.io/badge/python-%E2%89%A5%203.11-3776ab?style=flat&logo=python&logoColor=white">
<img alt="Linux and macOS" src="https://img.shields.io/badge/Linux%20%C2%B7%20macOS-supported-2ea44f?style=flat&logo=linux&logoColor=white">
<a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue?style=flat"></a>
</p>

Then give it a key. `prefix+space` is the suggested default; any herdr key works, including one
without the prefix such as `ctrl+alt+p`. Add this to `~/.config/herdr/config.toml` and run
`herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+space"          # change to taste, e.g. "ctrl+alt+p"
type = "plugin_action"
command = "leavemyyard.herdr-panorama.open"
description = "swarm view"
```

**Status: proof of concept.** It implements design 2a "Graph · squares are subagents"
(reference copy and render in [docs/design/](docs/design/)). The design, the data sources and the
open questions are in [docs/DESIGN.md](docs/DESIGN.md).

| Full screen (160 columns) | Side split (70 columns) |
|---|---|
| ![](docs/screenshots/wide-160x45.png) | ![](docs/screenshots/narrow-70x45.png) |

## Pick your setup

### 🗺️ Graph only: panorama in its own tab

The default. Panorama opens as the first tab, `◆ panorama`, and your key jumps back to it from
anywhere. Enter on an orchestrator goes to its pane; the full graph stays one key away. Details
in [Use it as your home tab](#use-it-as-your-home-tab).

```toml
# plugin config.toml (herdr plugin config-dir leavemyyard.herdr-panorama)
home_tab = true    # the default
cockpit = false    # the default
```

### 🧭 Cockpit: a slim list beside the agent you are working with

Panorama docks as a narrow panel on the left of an agent's tab, with that agent's chat on the
right. ↑/↓ in the panel switch the chat beside it while the keyboard stays in the panel; Enter
goes into the agent, and your open key brings you back. It also follows you when you switch to
another orchestrator's tab some other way. With herdr's own sidebar hidden, it becomes the agent
list. The panel's background shows where the keyboard is: `#14161b` while panorama has it,
`#101215` when the agent does. Still experimental; details in [Cockpit](#cockpit-experimental).

<div align="center">
<a href="docs/media/cockpit.mp4"><img src="docs/media/cockpit.gif" alt="The cockpit on demo data: ↓ through Billing API, Search refactor, Docs site and Release notes in the docked panel, the chat on the right changing each time; Enter goes into the Release notes agent, the open key returns to the panel, ↑ back to Billing API with its ?2 questions" width="920"></a>
</div>

```toml
# plugin config.toml
home_tab = true
cockpit = true
cockpit_width = 46            # panorama's columns on the left
cockpit_follow = true         # move with you to the orchestrator tab you switch to
cockpit_follow_tabs = "any"   # "single": only into tabs where the agent is the only pane
dock_keep_focus = true        # switching chats keeps the keyboard on panorama
dock_on_arrows = true         # ↑/↓ switch the chats (after 0.2 s); Enter goes into the agent
panel_colors = "fixed"        # or "herdr": take the panel colours from herdr's theme
# panel_bg = "#101215"        # the panel while the agent has the keyboard
# focus_bg = "#14161b"        # the panel while panorama has it
```

```toml
# ~/.config/herdr/config.toml
[ui]
sidebar_start_collapsed = true     # takes effect at herdr's next launch
sidebar_collapsed_mode = "hidden"  # no rail: panorama is the agent list
pane_outer_borders = false         # no frame around the pane area
pane_gaps = false                  # panorama and the agent share one divider
# pane_borders = "off"             # optional, see below

[[keys.command]]
key = "ctrl+alt+p"                 # a key without the prefix, one press from anywhere
type = "plugin_action"
command = "leavemyyard.herdr-panorama.open"
description = "swarm view"
```

**Recommended: turn herdr's pane borders down.** The cockpit is a split, so by default herdr
frames the pane area and gives panorama and the agent a border each with a gap between them.
Every one of those lines is a column taken from the 46-column panel (and from the agent's
chat), and the frame makes the panel look boxed in rather than part of the screen.
`pane_outer_borders = false` drops the outer frame and `pane_gaps = false` leaves a single
shared divider between panorama and the agent. `pane_borders = "off"` removes that divider
too, for the cleanest look; the trade-off is that panorama and the agent are then separated
only by their backgrounds, and every other split pane in herdr loses its border as well (and
with it the agent labels `show_agent_labels_on_pane_borders` draws there).

## Works great with herdr-inbox

Many agents means many questions. [herdr-inbox](https://github.com/LeaveMyYard/herdr-inbox) lets
them ask without stopping: each question is filed once, the agent keeps working, and you answer
every agent's questions from one popup. Panorama shows each open question as `?N` on its card;
`a` opens the inbox right on it, and the answer lands back in the agent's conversation.

<div align="center">
<a href="docs/media/herdr-demo.mp4"><img src="docs/media/herdr-demo.gif" alt="Four demo agents in panorama; the Release notes agent asks a question, its card and tab get a mark, the operator presses a, answers in the inbox popup, and the agent's pane shows the answer and the agent continuing" width="920"></a>
</div>

```sh
claude plugin marketplace add LeaveMyYard/herdr-inbox
claude plugin install herdr-inbox@herdr-inbox
herdr plugin install LeaveMyYard/herdr-inbox
```

## What it shows

- **Orchestrators**: every herdr pane running Claude Code, named by its tab, with its state
  (failed, blocked, needs you, working, reviewing, waiting, stale, idle, all done). **Waiting** ⠁ (a dot circling the cell):
  herdr says idle, but a Bash it put in the background or a Monitor watch is still running, so it
  will wake up by itself; its tasks are listed. **Stale** ◌: idle for longer than `stale_after`
  (12 h), probably forgotten. The card's left edge shows the
  state and blinks when something needs you, and a bar sweeps along the bottom while it works. It
  also carries `✗` failed,
  `!` blocked (permission prompt or question dialog) and `?N` open
  [herdr-inbox](https://github.com/LeaveMyYard/herdr-inbox) questions.
- **Threads** of the selected orchestrator: native Workflow runs, PR/issue review cycles, or one
  per operator prompt, each with its phases (`✓ author 09:02–09:20`, `» review r2 since 09:43`).
- **Agents** of the selected thread: status, role, model, last tool call, age, tool calls, open
  questions, last message. Finished agents fold into one card.

Status is colour and glyph: `?` needs answer · `!` blocked · `✗` failed · `⠋` running ·
`»` reviewing · `⠁` waiting on its background work · `○` queued · `✓` done · `✓?` done, inferred from silence (not recorded) ·
`~` lost (needs the Claude Code plugin) · `-` cancelled.

It never types into a pane. Its only actions are focusing a pane (`g`) and opening the
herdr-inbox popup (`a`, `i`) when that plugin is installed.

## Requirements

- **herdr ≥ 0.9.0** on Linux or macOS ([herdr.dev](https://herdr.dev)).
- **Claude Code**, any version that writes transcripts under `~/.claude/projects`
  (`CLAUDE_CONFIG_DIR` is honoured). The optional Claude Code plugin below needs plugin support.
- **[uv](https://docs.astral.sh/uv/)** on herdr's `PATH`; it brings its own Python 3.11+. The
  optional plugin's hooks use the system `python3`.

## Install (herdr plugin)

```sh
herdr plugin install LeaveMyYard/herdr-panorama
```

Bind a key in `~/.config/herdr/config.toml`, then `herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+space"
type = "plugin_action"
command = "leavemyyard.herdr-panorama.open"
description = "swarm view"
```

`leavemyyard.herdr-panorama.open-split` opens it in a side split instead of the full-screen popup.

## Use it as your home tab

By default panorama lives in its own tab, `◆ panorama`, first in the tab bar:

- herdr's startup hook opens it once. If it is already there it is reused,
  never opened a second time.
- The key (`prefix+space`) jumps back to it from anywhere, and opens it again
  if you closed it.
- **Enter** on an orchestrator, or **g** on any card, goes to that agent's pane.
  In the home tab, `a`/`i` open the inbox over panorama, and nothing closes it
  except `q`.
- **`+ New agent`** (the row under the orchestrator cards, or **n**) starts a new
  Claude Code agent. Fill in a name, a working directory and an optional first
  prompt. The name is also the tab label, and it is made to fit herdr's agent
  names (`Billing export` becomes `billing-export`).
- On submit panorama creates the tab, starts Claude Code there and, if you gave
  one, sends the first prompt once. This is the only time panorama types into a
  pane, and only on your submit.

<img src="docs/screenshots/new-agent.png" alt="The New agent form over the home tab" width="920">

To use the popup instead, set `home_tab = false` in the plugin's `config.toml`. The action
`leavemyyard.herdr-panorama.open-popup` always opens the popup.

### Cockpit (experimental)

With herdr's sidebar hidden, panorama can sit as the agent list on the left of the agent
you are working with. Turn it on in the plugin's `config.toml`:

```toml
cockpit = true        # default false while experimental
cockpit_width = 46    # panorama's columns on the left
cockpit_follow = true # follow you to whichever orchestrator tab you switch to (default true)
cockpit_follow_tabs = "any" # "single": only into tabs where the orchestrator is the only pane
dock_keep_focus = true # Enter in the docked list keeps the keyboard on panorama (default true)
```

- **Enter** on an orchestrator (or **g**, or a click on the selected card) moves panorama's
  own pane into that agent's tab, on its left, and focuses the agent. The agent never
  moves: closing a tab closes that agent's tab, as always, and panorama with it.
- **Browse the chats.** In the docked list, ↑/↓ switch the chats themselves: once the
  arrows rest (0.2 s; holding one docks once), panorama docks next to the highlighted
  orchestrator and keeps the keyboard. A click does the same at once. **Enter** (or **g**)
  goes into the agent. `dock_on_arrows = false` brings back Enter-to-dock (keeping the
  keyboard); `dock_keep_focus = false` makes Enter go into the agent every time.
- **It follows you.** Switch tabs any other way (the tab bar, `prefix+1..9`, a
  notification, `herdr agent focus`), and when you land on a Claude orchestrator's tab the
  panel moves there too, beside the agent. You stay exactly where you clicked. It never
  moves for tabs without an orchestrator, other workspaces, while you are in panorama
  itself, in the New agent form or zoomed, and it acts once per switch. By default it
  also docks into tabs you split (`cockpit_follow_tabs = "single"` keeps it out of those).
  With two clients attached to one herdr session, either one's tab switches move it, so
  turn `cockpit_follow` off there.
- **z** zooms panorama to the whole tab (the full graph) and back to the panel.
- `prefix+space` takes you back to the list wherever it is; if panorama was closed, it
  opens again in its home tab.
- In a tab with several panes, panorama docks left of the agent's own pane.
- Docked, panorama shows the compact list from design 3a with design 9e's coloured tiles:
  per orchestrator its tab number, a state tile, `?N` and one tile per thread; the docked one
  lists its threads (a tile each) with their agents instead.
- The panel's background shows whether panorama has the keyboard: `#14161b` while it does,
  `#101215` when not. `panel_bg` / `focus_bg` in panorama's config change them;
  `panel_colors = "herdr"` takes them from herdr's theme instead (`[theme]` and
  `[theme.custom]` in herdr's `config.toml`, with `focus_tint`).

  <img src="docs/screenshots/dock-app.png" alt="The docked cockpit panel" width="300">
- `cockpit = false` stops docking at the next Enter, and brings panorama back to its own
  first tab at the next `prefix+space` or start.
- If you split the home tab and run something next to panorama, docking renames that tab
  to the agent's name first, so nothing is left under panorama's label.

## Optional: Claude Code plugin

The view works without it. The plugin adds, for the sessions where it is installed:

- `SubagentStart` / `SubagentStop` hooks that record exact start and stop times. Agents that
  ended then show as a confirmed `done` instead of `done?`. An agent that started but never
  stopped, and whose Claude process is gone, shows as `lost`.
- an MCP tool, `report_thread(thread_id, title, phase, phases_done, next, status, agents)`, that
  orchestrators call to declare threads and phases (the same format as `panorama emit`);
- one short line of session instructions (inside herdr only) asking orchestrators to use it.

```
/plugin install herdr-panorama --marketplace LeaveMyYard/herdr-panorama
```

or `claude plugin marketplace add LeaveMyYard/herdr-panorama` then
`claude plugin install herdr-panorama@herdr-panorama`. The hooks need `python3` on `PATH` (stdlib only); the MCP server needs `uv`.

## Configure

`config.toml` in the plugin's config dir (`herdr plugin config-dir leavemyyard.herdr-panorama`):

```toml
done_window = "1h"   # how long finished threads stay visible (seconds, or "90m", "2h30m")
home_tab = true      # panorama in its own first tab (false: the key opens a popup)
cockpit = false      # experimental: panorama docked left of the selected agent (see above)
cockpit_width = 46   # the list's width in the cockpit
cockpit_follow = true # the cockpit follows you to the orchestrator tab you switch to
cockpit_follow_tabs = "any" # or "single": follow only into one-pane orchestrator tabs
dock_keep_focus = true # docked list: switching chats keeps the keyboard on panorama
dock_on_arrows = true  # docked list: ↑/↓ switch the chats; Enter goes into the agent
panel_colors = "fixed" # docked panel background: "fixed" (#14161b focused, #101215 not) or "herdr" (herdr's theme)
# panel_bg = "#101215"  # the unfocused background, in either mode
# focus_bg = "#14161b"  # the focused background, in either mode
focus_tint = 0.15      # "herdr" mode: how far focus moves panel_bg toward the theme's active_row_bg
machines = "all"     # herdr's saved SSH machines to show: "all", "none" or ["label", ...]
highlight_unseen = true # pulse what failed, asked or finished since you last looked
animation_fps = 20   # how smoothly pulses and blinks fade (2..30): lower it on a slow terminal or remote link
stale_after = "12h"  # an orchestrator idle this long (nothing in flight) reads "stale"
```

**Unseen.** A new failure, a new question or a finished agent makes its card pulse slowly in
the event's colour (red, amber, green; the most urgent wins), and its thread and orchestrator
with it. A finished agent makes them pulse more quietly. In the docked panel the orchestrator's
two lines and the thread glyph pulse. It stops when you click the card, press Enter on it,
go to or dock next to its orchestrator, or focus that pane in herdr. The marks survive a
restart and are shared by every panorama of the machine.

**Other machines.** Agents on herdr's saved SSH machines (`herdr machine list`) appear in the
same view under a faint `─ <machine> ─` divider. Enter on one focuses it on that machine and
tells you to switch herdr there. Panorama reads their herdr state through `herdr --machine`
and their Claude transcripts over ssh (batch mode, never prompting). A machine that does not
answer shows as stale or unreachable and never slows the local view. See
[docs/REMOTE.md](docs/REMOTE.md).

The default is 1 hour. The `PANORAMA_DONE_WINDOW` environment variable overrides it.

## Use

| Key | Action |
|---|---|
| `↑` `↓` / `j` `k` | Move in the focused column |
| `→` `l` / `enter` | Open the next column (`enter` on a folded card unfolds it) |
| `←` `h` | Back |
| `g` | Go to the orchestrator's pane |
| `a` | Answer: herdr-inbox opened on that agent's (or orchestrator's) questions, the pane for blocking prompts. Click: `a answer` in an agent card, `?N` on an orchestrator card |
| `i` | herdr-inbox |
| `t` | Show the transcript path |
| `f` | Filter: all / running / needs me |
| `n` | New agent (also Enter on the `+ New agent` row) |
| `q` `esc` | Close (in the home tab only `q`) |

The mouse works too, in both layouts: click a card to select it, click the
selected card again to go to its pane, click `✓ n complete` / `n finished` to
unfold, `+ New agent` for the form, the header's `all` / `running` /
`needs me` to filter, and `g` / `t` / `a` in an expanded agent card to act.
The wheel scrolls a column. A card that a refresh just moved under the pointer
ignores clicks for a second.

From a shell:

```sh
uv run panorama snapshot            # text summary of the derived model
uv run panorama snapshot --json     # the full model
uv run panorama snapshot --counts   # numbers only
uv run panorama tui
```

## FAQ and troubleshooting

**The key does nothing.** Bind it first (top of this page), then `herdr server reload-config`.
`herdr plugin list` should show `leavemyyard.herdr-panorama` enabled, and `herdr plugin log`
shows its action and startup logs, including a missing `uv`.

**An agent is missing.** Panorama lists the panes where herdr detects Claude Code
(`herdr agent list`) and reads their transcripts from `~/.claude/projects`. A pane whose agent
herdr does not see does not get a card.

**Agents show `done?` or `lost`.** Without the optional Claude Code plugin, an agent's end is
inferred from silence. Install it (above) for exact start and stop times.

**Does it change my agents or send anything anywhere?** It reads herdr's state and the local
transcripts, focuses panes and opens the inbox when you ask. It types into a pane only when you
submit the New agent form. Other machines are read over your own SSH; nothing goes to a hosted
service.

**I don't want the `◆ panorama` tab.** Set `home_tab = false`; the key then opens a popup.

**The cockpit panel sits next to herdr's sidebar.** Hide herdr's sidebar with the `[ui]` settings
in [Pick your setup](#-cockpit-a-slim-list-beside-the-agent-you-are-working-with), then restart
herdr.

## Development

```sh
uv sync
uv run pytest                            # tests (fake herdr, captured fixtures)
uv run pytest tests/make_screenshots.py  # regenerate docs/screenshots (needs Chrome)
uv run --with pillow python docs/screenshots/hero_gif.py  # the README GIF (needs Chrome and ffmpeg)
```

`tools/accuracy.py` compares agent statuses against an independent checker on real sessions
(read-only; see DESIGN.md "Accuracy").

## Credits

The session scanner (`src/herdr_panorama/scan/`) is a Python port of the discovery, linking,
record parsing and tailing of [zoetrope](https://github.com/furkankly/zoetrope) v0.2.0, Copyright (c) 2026 Furkan Kalaycioglu,
MIT ([LICENSES/zoetrope-MIT.txt](LICENSES/zoetrope-MIT.txt)), with fixes for resumed, killed and
nested agents.

## License

MIT
