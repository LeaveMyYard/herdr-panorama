# herdr-panorama as a herdr-app mod: implementer's spec

This spec turns the Claude Design file `Herdr App Panorama v2.dc.html` (in this folder) into
build rules. The frames:

| File | What it shows |
|---|---|
| `v2-a-window.png` | The herdr-app window at the app's 1.2 zoom. The `panorama` mod replaces the Agents panel; everything else is the shipped mods. |
| `v2-b-sidebar.png` | The sidebar at 2x with every fold open, plus a legend of every mark. |
| `v2-c-peek.png` | 2x crop: the Peek popup on a subagent that has no pane. |
| `v2-d-graph-tab.png` | A space was clicked. Its **Swarm** tab (mod `panorama-graph`) is the first tab and is selected. |

The HTML is a reference copy. It needs the Claude Design runtime `support.js` to render, and
that file is not copied here. The PNGs are the source of truth for the look.

All sizes below are **logical px** (gpui units, before the user's `zoom`). Colours are
**`theme()` names**; hex values are the default dark theme, sampled from the user's screenshot,
and are given only for reference. Use the names in code, so light themes work too.

---

## 1. Layout

`layouts/main.json`, with one change to the sidebar column:

```
sidebar (column, px 320)
├─ brand        block 46px  mod workspace-chrome          (unchanged)
├─ sidebar-lists column
│  ├─ spaces    block, sized to content (31 + rows·30 + gaps·4 + 14), min 90   mod spaces (patched, §5)
│  └─ agents    block weight 1, min 90                    mod panorama   ← replaces mod "agents"
└─ sidebar-footer block 69px mod workspace-chrome {footer:true} (unchanged)
```

- Sidebar width: **320** (the default is 268; the user runs about 354). The panel must still
  work at 268; truncation rules are below.
- Spaces no longer takes half the column. Give it a fixed px equal to its content (3 spaces
  → 143) so the swarm gets the rest.
- Narrow rail (`sidebar.size.px < 160`): fall back to the shipped agents rail (numbers + the
  compact status), with no tree.

## 2. Colours (`theme()` names)

| Use | Name | Default dark |
|---|---|---|
| Sidebar / bars background | `chrome` | `#101114` |
| Work area behind cards | `canvas` | `#131518` |
| Pane / graph background | `background` | `#17191d` |
| Dividers, card borders | `border` | `#ffffff14` |
| Row hover | `hover` | `#ffffff0e` |
| Selected row (focused orchestrator, selected space) | `selected` | `#ffffff17` |
| Peeked row, Peek chip, popup buttons | `raised` | `#232427` |
| Popup background | `popover` | `#1c1d20` |
| Tree lines, graph connectors, "next" phase bar | `input` | `#ffffff24` |
| Names | `foreground` | `#eeeeef` |
| Subagent names, secondary text | `muted` | `#98999b` |
| Titles after "·", ages, counts, headers | `dim` | `#696a6d` |
| Focused pane border | `accent` | `#96b2d7` |
| Working: spinner, moving bar, "now" phase bar, Working header | `working` | `#e5c07b` |
| Needs you: chips, left edge, failed/waiting phase bar, Needs you header, needs text | `blocked` | `#f0705f` |
| Done dot, Done header, folded-done count | `done` | `#76a8ff` |
| Queued/idle ring | `idle` | `#65696f` |
| Reviewing chip | `ansi5` | `#b9a0e2` |
| Done phase bar | `dim` at ~85% | `#5c5d61` |

Tints: a chip background is the colour plus alpha `2b` (about 17%), e.g. `t.blocked + "2b"`.
The shipped mods use `26`; either is fine. The moving bar's track is `t.working + "1c"`.

Amber means working everywhere in herdr-app (tab pills, pane header), so panorama follows
that: **working is amber, not the TUI's teal**. Everything that needs the operator uses the
`blocked` hue. Failed is told apart by being a **solid** chip.

## 3. Status marks

Status words map from panorama's model (`model.py` codes) to one mark each. A mark sits at the
right end of a row or graph node, 6px after the previous element.

| Code | Meaning | Mark | Spec |
|---|---|---|---|
| `run` | running / working | spinner | `mods/workspace-chrome/spinner.svg`, size 12, `text_color(t.working)`, `.spin(900)`. Left margin 8, right 1. |
| `rev` | reviewing | `»` chip | h16, min-w16, px4, radius 4, bold 10.5, text `ansi5`, bg `ansi5 + "2b"` |
| `wait` | waiting for an answer | `?N` chip | N = open questions (inbox) for that agent, summed upward. Text `blocked`, bg `blocked + "2b"` |
| `blk` | blocked on a permission | `!` chip | as `?N` |
| `fail` | failed | `✗` chip, **solid** | bg `blocked`, text `chrome` (dark ink) |
| `done` (subagent) | finished | `✓` | 11px text `dim`, 14px wide box |
| `done` (orchestrator, unseen) | herdr status `done` | dot | 7×7 circle, bg `done`, margins 0 3 0 9 (same as the shipped mod) |
| `q` / idle | queued, idle | ring | 7×7 circle, 1.5px border `idle` |
| `cxl` | cancelled | `-` | 11px text `dim` (not drawn in the design; use the ✓ slot) |
| `bg` | waiting on background tasks | ring + small spinner? | Not drawn. Use the ring until it has a design. |

**Aggregation (most urgent wins):** `fail > blk > wait > rev > run > q > done`.

- **Thread row / node:** one mark, the most urgent among all its agents and their
  descendants. `?N` sums the questions.
- **Orchestrator row / node:** first one chip per needs kind present below it, in the order
  `✗`, `!`, `?N`. Then the orchestrator's own herdr status mark: spinner for `working`, dot
  for `done`, ring for `idle`. When its own status is `blocked`, the `!` chip already says it,
  so don't draw a second one.
- **Agent row / node:** its own status mark.

## 4. Panorama sidebar mod (`mods/panorama`)

### 4.1 Header

Same as the shipped agents mod: h41, padding 14 16 5. "Agents" 11.5 medium `dim`, plus the
`clock` sort icon 14 on the right. Keep the label "Agents"; the slot is still the agents slot.
The list below has px8 and `overflow_y_scrollbar().mode("always")`.

### 4.2 Sections

These are the shipped priority sections, in the shipped order: **Needs you → Done → Working →
Idle**.

- Each orchestrator goes into the section of its **most urgent descendant**, like shipped
  parents. Needs you = any `fail/blk/wait` below it, or its own herdr status `blocked`. Done =
  own status `done`. Working = `working`. Idle = `idle`.
- Header row: h30, pt8, px8, 11 medium. Label colour: Needs you `blocked`, Done `done`, Working
  `working`, Idle `dim`. The count follows 6px later, in `dim`. Clicking folds the section
  (shipped behaviour).
- Pinned agents keep the shipped "Pinned" group on top.
- Inside a section, sort like shipped: urgency, then last change, then `seq`.

### 4.3 Row types

All rows: `px8`, `rounded(8)`, hover `t.hover`, no gap between rows (tree lines must join).

| Row | Height | Content, left to right |
|---|---|---|
| Orchestrator | 30, text 13 | harness icon 16 (mr 9) · `space_name` `foreground` (max-w 110, truncate) · `· title` `dim` (flex 1, truncate, ml 5) · fold chevron (only when folded) · age 11 `dim` (ml 8) · marks |
| Thread | 26, text 12.5 | tree segments · name **500** `foreground` (flex 1, truncate) · phase bars (ml 8) · one mark |
| Agent / subagent | 26, text 12.5 | tree segments · harness icon 14 (mr 7) · role `muted` (max-w 110) · `· activity` `dim`, or `blocked` when it needs you (flex 1, truncate, ml 5) · fold count (if folded) · marks |
| Summary | 26, text 12.5 | tree segments · `✓ N done` `dim` · `· role, role` `dim` |

- **Selected:** the orchestrator whose pane is focused gets `t.selected`, the same as the
  shipped `a.focused`. When a Swarm graph tab is focused (§6), no row is selected.
- **Name:** in the priority view the orchestrator's name is `space_name`, and the detail is
  the shipped `label(a)` (named tab or terminal title). The shipped de-duplication rules
  carry over.
- **Idle orchestrator:** its name goes `muted` (shipped).
- **Age:** time since the last status change (the shipped `changed` store). On hover it is
  replaced by the pin button, as shipped.

### 4.4 Tree lines

They follow the shipped `subRow` style, made continuous.

- Each depth level is a 13px-wide segment, stretched to the full row height. The line is 1.5px
  `t.input`, drawn at x=7 inside the segment.
- Segment kinds:
  - `│`: a vertical line from top −2 to the bottom
  - `├`: that vertical plus an elbow
  - `└`: the elbow only
  - blank
- The elbow is a box from top −2, 15px tall and 6px wide, with left and bottom borders and a
  bottom-left radius of 6. Its bottom lands at the row's vertical centre (13).
- After the last segment (`├`/`└`) leave a 4px gap before the content.
- Geometry check: the first level's vertical sits at x=15 in the row, under the orchestrator
  icon's centre (8+8). Each deeper level's vertical sits under the parent agent icon's centre.
- In gpui, draw each segment as a fixed 13px box with one or two absolute children.

### 4.5 Phase bars (threads)

- One bar per phase: 8×4, radius 2, gap 2.
- Colours by phase state:
  - done: `#5c5d61` (`dim`)
  - now: `working`
  - failed: `blocked`
  - waiting on the operator: `blocked`
  - next / skipped: `input`

### 4.6 Default depth and folding

Show depth only where there is news.

1. **Orchestrators in Needs you and Working** are open and show **all their threads**.
2. **Under a thread**, agents appear only when they or a descendant need you
   (`fail/blk/wait`). Other agents are hidden. The thread's single mark still shows they run.
3. **Finished agents** under an open thread collapse into one summary row: `✓ N done ·
   role, role`. This row sits last among its siblings.
4. **Subagents of a shown agent** are shown only if some of them need you. Otherwise the
   agent row carries a fold count `› N` (chevron-right 11 + N, 11px `dim`).
5. **Finished threads** of an open orchestrator collapse into `✓ N tasks done · names` as its
   last child.
6. **Orchestrators in Done and Idle** start folded. A folded orchestrator shows chevron-right
   plus its thread count, in the colour of its most urgent descendant: `blocked` if anything
   needs you, `done` if it's done, else `dim`.
7. Clicking a chevron, or the harness icon of a row with children (shipped), toggles that
   row's fold. User folds are kept in memory per address and override rules 1–6 until the
   next status change of that subtree.
8. Frame B is the fully open state (every fold toggled open). It's for the design review, not
   a mode, though a "expand all" keybinding would produce it.

### 4.7 Motion

All motion runs off one timer at 50 ms (20 fps): `cx.timer.every(50, cx => cx.notify())`. Run
it only while something animates (a needs-you edge or a moving bar is on screen); otherwise
keep the shipped 30 s timer. Spinners use gpui's own `.spin(900)` and need no timer.

| Effect | Where | Spec |
|---|---|---|
| Needs-you edge | Orchestrator rows with any needs chip (or own `blocked`); agent rows that are `fail/blk/wait`; same on graph nodes | Absolute, left 0, top 6, bottom 6, width 2, radius 1, bg `blocked`. Opacity = `0.3 + 0.7·(0.5 − 0.5·cos(2π·t/1600ms))`, a 1.6 s ease in and out that never fully disappears. |
| Moving bar | Orchestrator rows (and graph nodes) with any `run` or `rev` descendant | Track: absolute, left 33 (under the text), right 8, bottom 1, h2, radius 1, bg `working + "1c"`, clipped. Segment: 28% of the track width, bg `working` at 0.85 opacity. Position: `p = (t mod 2400ms)/2400`, eased in-out quad (`p<.5 ? 2p² : 1−(−2p+2)²/2`), left = `−28% + eased·128%`, so it sweeps fully across. On graph nodes the track is left 12, right 10, bottom 2. |
| Spinner | Every `run` mark, tab pill, pane header | `.spin(900)` |

### 4.8 Clicks

| Row | Click | Notes |
|---|---|---|
| Orchestrator | `run("pane.focus", { pane: a.pane })` | Same as the shipped row. |
| Agent that has a pane (`terminals()` entry with an address) | `pane.focus {pane}`; for a pane-less terminal sub-agent, `terminal.peek {terminal}` | This matches the shipped subRow. |
| Claude Task subagent (no terminal) | Open the **Peek** popup (§4.9) | On hover, a `Peek` chip appears before the mark: h18, px6, radius 5, 10.5 medium, `foreground` on `raised`. |
| Thread | Toggle its fold | Threads have no pane. |
| Summary row | Expand those finished agents | |
| Chevron / fold count | Toggle the fold | `cx.stop_propagation()` |
| Section header | Fold the section (shipped) | |

`setAgentOrder` keeps the shipped behaviour: orchestrator addresses in display order.

### 4.9 Peek popup (pane-less subagent)

`openOverlay({ mod: "panorama", kind: "popup", width: 284, props: { peek: <agent id> } })`,
anchored at the pointer / right of the row.

- Container: padding 12, gap 9, text 12.5, bg `popover`, 1px border `#ffffff1f`, radius 10.
- Header: harness icon 16 · role 600 `foreground` · thread name `dim` (flex, truncate) · status
  mark.
- Meta (11.5 `dim`, line 1.45): `Failed 1m ago · Claude Code · sonnet · 9m · 23 tools`, then on
  a new line `Subagent of <space · orchestrator title>. It has no pane of its own.`
- Error box, only when there is an error: mono 11.5/1.5, text `#f6a99e` (blocked, lightened),
  bg `blocked + "1c"`, border `blocked + "40"`, radius 7, padding 7 9.
- Label "Last message" 11 `dim`. Under it, the agent's `last_message`, 12.5 `muted`.
- Buttons, each h28, px10, radius 7:
  - **Open orchestrator ⏎**, solid: bg `solid`, text `solid_foreground`. Runs `pane.focus` on
    the owning orchestrator.
  - **Transcript t**: bg `raised`. Opens the transcript, e.g. `panorama` writes a temp file
    and runs `mod.open` on a viewer, or `exec` an editor.
- While the popup is open, the row gets bg `raised`.

## 5. Spaces mod additions (patch to `mods/spaces`)

Each space row keeps the shipped tile, name and git tail, and gets two things after the git
tail:

- **Needs-you count chip:** the number of items needing you in that space (`fail` + `blk` +
  `wait` agents, counted over the whole swarm, subagents included). It uses the `?N` chip
  style (`blocked` on `blocked+"2b"`), shows only the number, and has ml 8 after the repo
  text.
- **Working spinner:** when any agent in the space works. Size 12, `working`, `.spin(900)`, ml 8.
- If the git tail is long, truncate it first; the chip and spinner never truncate.
- The compact rail is unchanged.
- **New click behaviour:** clicking a space selects it **and opens its Swarm tab** (§6).
  `space.select` alone stays on drag-to-reorder and keyboard selection.

## 6. Swarm graph tab (`mods/panorama-graph`)

### 6.1 Opening

On a space click (patched spaces mod, or a `panorama` command the spaces mod calls):

1. `run("space.select", { id })`.
2. If the space has no Swarm tab: `run("tab.new", { space: id, name: "Swarm", mod:
   "panorama-graph", props: { space: id } })` (a tab whose only pane is a mod; documented in
   `herdr.d.ts`), then `run("tab.move", { id: <new tab id>, index: 1 })` to make it the first
   tab. Find the new id in `spaces()` by the name "Swarm" and a mod pane in `panes()`.
3. Select the Swarm tab.

Remember per space that the user closed it, and don't recreate it until the next space click.

### 6.2 Tab pill (patch to `mods/tabs`)

A mod tab has no `program`. Draw instead:

- A **2×2 glyph of status squares**: 5×5 each, radius 1.5, gap 2. Squares, left to right, top
  to bottom:
  - top-left: `blocked` if the space has needs, else `dim`
  - top-right: `working` if anything works, else `dim`
  - bottom-left: `dim`
  - bottom-right: `done` if anything is done-unseen, else `dim`
- The name "Swarm".
- The needs-you count chip, same as in §5.
- Pill size and states are those of shipped tabs: h29, px10, gap 7, radius 7, text 12.8;
  active is bg `#1e1f22` with an inset 1px `#ffffff1a` ring (the shipped selected look).
- No spinner and no close icon while inactive. While active it can show the shipped close.

### 6.3 Pane card

The shipped pane card (border `accent` when focused, radius 10, bg `background`) with a 30px
header: glyph · "Swarm" 500 · `dim` text `<space> · N orchestrators · N threads · N agents` ·
the orchestrator-style aggregate marks for the whole space.

### 6.4 Graph layout

The graph runs left to right, the TUI 2a graph restyled to herdr-app. It shows **only this
space's orchestrators**.

- Graph padding: 18 22. Scrolls both ways when larger than the pane.
- Four columns, each **264** wide with **44** between them: Orchestrators, Threads, Agents,
  Subagents.
- Column headers sit at the top: 11.5 medium `dim`, the count 6px later in `#4d4e52`. The
  first row starts at y=30.
- **Rows:** every node is **52** tall on a **62** pitch. A leaf (an agent without subagents,
  or each subagent) takes one row. A parent node is placed at the top row of its subtree.
  Orchestrators are separated by an extra 20.
- **Subagents of subagents** go in the same column, indented 26 per extra depth. They are
  joined to their parent by an elbow: a vertical at parent.x+13 from the parent's bottom down
  to the child's centre, then horizontal to the child.
- **Connectors:** 1.5px `input`. From the parent's right edge at y = top+26, a horizontal line
  of 22 reaches a spine. The spine runs vertically to the last child's y+26. A horizontal stub
  goes to each child's left edge.
- **Node card:** bg `#1d1e21` (raised, slightly darker), 1px `border`, radius 8, padding 8 10
  0 12, two lines with gap 5.
  - Line 1 (h18): harness icon (16 on orchestrators, 14 on agents) · title 13 (500 for
    orchestrator and thread titles; role in `muted` for agents) · marks.
  - Line 2 (h15, 11.5): `dim` text, or `blocked` when the node needs you. Threads show their
    phase bars (§4.5) before the text, mr 7.
  - Orchestrator line 2: `<harness> · <age> · tab <n>`.
  - Thread line 2: the current phase and its note (`CI · 3 failing`, `review r2`, `author ·
    waits on Q14`, `done`).
  - Agent line 2: the activity.
- **Subagent nodes** use bg `#19191c` and a `muted` title: pane-less, matching the sidebar.
- A finished orchestrator's whole subtree is drawn at **0.55 opacity**.
- **Motion:** the same edge and moving bar as the sidebar (§4.7), on nodes.

### 6.5 Graph clicks

These are exactly the sidebar's:

- An orchestrator node, or an agent node with a pane: `pane.focus`. That switches from the
  Swarm tab to that agent's tab.
- A pane-less subagent: the Peek popup.
- A thread node: toggles collapsing its agents into a `✓ N done` node.

Clicking a node never types into a pane (panorama stays read-only).

## 7. Harness icons

- Use `agent.program.icon` with `program.tint`:
  `p.tint ? svg(p.icon).size(s).text_color(t.foreground) : image(p.icon).size(s)`. This is the
  shipped `icon()` helper.
- Sizes: 16 on orchestrator rows and nodes, 14 on everything below, 14 in tab pills.
- Claude Task subagents have no terminal, so they have no `program`. Use the parent session's
  harness (Claude Code for Task subagents), i.e. `icons/claude.svg`.
- Agents with an unknown harness use `icons/agent.svg`, tinted.

## 8. What a real mod could not do as drawn (or needs extra work)

1. **Threads, phases and Claude Task subagents are not in herdr-app's data.** `agents()` and
   `terminals()` only know terminals. The mod has to read panorama's model:
   `exec("panorama", ["snapshot", "--json"])` on a timer, or after `watchFolder` on
   `~/.claude/projects/<slug>/` and the panorama events dir. It then has to match orchestrators
   to herdr agents by pane / session id. Both APIs exist in `herdr.d.ts`.
2. **`terminal.peek` needs a terminal.** A Claude Task subagent has none, so Peek is the mod's
   own `openOverlay` popup, not the app's peek drawer. Transcript viewing needs a viewer mod
   or an external editor.
3. **The status bar (shipped `workspace-status`) still counts only blocked panes.** It reads
   "1 needs you" while the sidebar shows 4 needs-you items. Matching it means patching that
   mod to read the same snapshot.
4. **Animation cost.** The eased edge and the moving bar mean re-rendering the panel at 20 fps
   while they're visible. Keep the timer conditional (§4.7). If it's still too heavy, drop the
   bar to 8 fps; the eased edge needs about 20 fps to look smooth.
5. **Reviewing uses `ansi5`**, not a named status colour, so its contrast depends on the
   theme.
6. **Tree lines** are CSS pseudo-elements in the HTML. In gpui each segment is a 13px box with
   one or two absolute children; the shipped `subRow` already does a simpler version.
7. **The graph tab's position.** `tab.new {mod}` and `tab.move {id, index}` exist, but
   nothing pins the tab: the user can drag it away. `tab.new` doesn't return the new id, so
   the mod has to find it in `spaces()` afterwards. Check whether `index` is 0- or 1-based
   before relying on it.
8. **The tab pill glyph** needs a patch to the shipped `tabs` mod, which today draws
   `program.icon` only. Ship the patched tabs mod with panorama, or ask upstream for a
   mod-tab icon field.
9. **Spaces sizing** (content-sized instead of half the column) is a layout JSON change.
   Users with their own layouts have to apply it themselves.
10. **Fonts.** The HTML uses Inter and JetBrains Mono as stand-ins. The app draws with its
    configured UI and terminal fonts.
