# herdr-panorama as a herdr-app mod: implementer's spec

This spec turns the Claude Design file `Herdr App Panorama v2.dc.html` (in this folder) into
build rules. The frames:

| File | What it shows |
|---|---|
| `v2-a-window.png` | The herdr-app window at the app's 1.2 zoom. The `panorama` mod replaces the Agents panel; everything else is the shipped mods. |
| `v2-b-sidebar.png` | The sidebar at 2x with every fold open, plus a legend of every mark. |
| `v2-c-peek.png` | 2x crop: the Peek popup on a subagent that has no pane. |
| `v2-d-graph-tab.png` | A space was clicked. Its **Swarm** tab (mod `panorama-graph`, based on Swarm View 5 deep) is the first tab and is selected. The thread has focus, so the columns orchestrators → depth 2 show and depths 3–5 are off to the right. |
| `v2-d2-graph-scrolled.png` | The same tab after → → → → onto a depth-4 agent: the window scrolled right to depths 2–5. The breadcrumb names the hidden columns; the selected card is open with its question and actions. |

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
The shipped mods use `26`; either is fine. The moving bar has no track of its own: it lights the border it runs on (§4.7).

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
| `bg` | waiting on background tasks | lavender orbit | Steady, no sweep, no blink. Labelled **Waiting**. A root takes it when its feed `bg` list is non-empty or an agent at any depth has status `bg` (the TUI's `n_bg`), and nothing more urgent holds (no needs, nothing running), whether herdr says `idle` or `done`. `bg` is what the writer believes is live: a task it cannot check (a remote root, an unreadable process tree, a Monitor task) stays listed for up to 24h (`BG_CAP`), and unlike the TUI's `waiting 3h` the app shows no age. Not to be confused with `wait`, the `?N` state: the word `waiting` is an alias of `wait`, so roots carry the code `bg`. |

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
Waiting → Idle**.

- Each orchestrator goes into the section of its **most urgent descendant**, like shipped
  parents. Needs you = any `fail/blk/wait` below it, or its own herdr status `blocked`. Done =
  own status `done`. Working = `working`. Waiting = nothing above, with background work (`bg`, §3),
  which takes it out of Done and Idle. Idle = `idle`.
- Header row: h30, pt8, px8, 11 medium. Label colour: Needs you `blocked`, Done `done`, Working
  `working`, Waiting the `bg` lavender, Idle `dim`. The count follows 6px later, in `dim`. Clicking folds the section
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
- After the last segment (`├`/`└`) leave a 4px gap before the content. A sidebar first-level row
  draws the boxed 16px tile instead: no gap, margin −1, so the connector runs into the tile, the
  tile is centred on its children's vertical and the title stays where the 11px glyph left it.
  Level 2 and deeper stay bare.
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
| Needs-you edge (**is the left border**) | Orchestrator rows with any needs chip (or own `blocked`); agent rows that are `fail/blk/wait`; graph cards that need you | The edge is the row's or card's own left border lit in `blocked`, not a separate stroke beside it. **Graph card:** a 2px overlay centred on the card's 1px left border line (left −1.5 relative to the padding box), from top 8 to bottom 8 so it stops short of the rounded corners. **Sidebar orchestrator row:** the row's left side, 2px wide, from top 6 down to the bottom where it meets the hairline bottom border (below), so the two read as one corner of a box. **Sidebar agent row** (no box): 2px at the row's left, top 6 to bottom 6. Opacity = `0.3 + 0.7·(0.5 − 0.5·cos(2π·t/1600ms))`, a 1.6 s ease in and out that never fully disappears. |
| Moving bar (**runs on the bottom border**) | Orchestrator rows and graph orchestrator cards with any `run` or `rev` descendant; graph agent cards that are `run`/`rev` | A lit segment travels along the bottom border line itself; there is no separate track. **Graph card:** a 2px strip centred on the card's 1px bottom border (bottom −1.5), from left 10 to right 10 (clear of the corners), clipped. **Sidebar card:** the light is on the card's own 1px bottom border, on the last row of an open card, below every tree row. It spans the full border-box width, corner to corner, and bends round the rounded corners. Segment: 28% of the strip (sidebar: of the card), `working` at 0.85 opacity. Position: `p = (t mod 2400ms)/2400`, eased in-out quad (`p<.5 ? 2p² : 1−(−2p+2)²/2`), left = `−28% + eased·128%`, so it sweeps fully across and off. |
| Spinner | Every `run` mark, tab pill, pane header | `.spin(900)` |

**Drawing it in gpui:** draw both as absolutely positioned children laid on the border line, not
inside the content.

- **Card:** keep the 1px `border` on the card (`.border_1().border_color(t.border)`, `relative()`)
  and don't clip it with `overflow_hidden()`; clip only the text rows inside.
- **Edge:** `div().absolute().left(px(-1.5)).top(8).bottom(8).w(2).rounded(1).bg(t.blocked).opacity(a)`.
- **Bar:** `div().absolute().left(10).right(10).bottom(px(-1.5)).h(2).overflow_hidden()`, holding
  the segment `div().absolute().left(relative(x)).w(relative(0.28)).h_full().bg(t.working).opacity(0.85)`.
- **Sidebar card:** gpui paints an element's border over its children, so the bar is the card's
  sibling in a `relative()` holder, not its child. An 8px strip at the holder's bottom holds the
  moving 28% clip window; in it, a copy of the card's bottom outline (1px left, right and bottom
  borders, radius 8, 16px tall so gpui keeps the radius) stays pinned to the card.
- **Timer:** re-render on the 50 ms timer and set `left` / `opacity` from the clock. No element
  changes size, so layout stays stable.

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

## 6. Swarm graph tab (`mods/panorama-graph`, based on Swarm View 5 deep)

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

### 6.4 Graph model: Swarm View 5 deep

The graph is **Swarm View 5 deep** (design turn 5, direction 5d), restyled to herdr-app. It is
the model the TUI ships: `docs/design/swarm-deep.js` and `render-5d.mjs`, docs/DESIGN.md
"Subagents to any depth", and `src/ui/graph/columns.rs` for the arrow keys. Only the look
changes; structure and behaviour are the same.

**One column per depth:**

```
Orchestrators | Threads | Depth 1 | Depth 2 | … | Depth N
 (this space)   (of the    (agents of the selected thread, by spawn depth)
                selected
                orch.)
```

- **Orchestrators:** this space's orchestrators only (`props.space`).
- **Threads:** the selected orchestrator's threads.
- **Depth columns:** there is one per depth of the **selected thread's** spawn tree, as deep as
  that tree goes. Depth 1 holds the agents the orchestrator spawned; depth k+1 holds the agents
  that depth-k agents spawned.
- **Card placement:**
  - A card sits level with the agent that spawned it, or below the cards already in its
    column.
  - An agent with children starts below the previous group's last child, so two groups'
    connector lines never share rows. This is 5d's `place()`: `y = max(cur[d], parentY,
    trunk[d+1] − mid + 16)`.
- **Folding finished agents:**
  - Finished children fold into one **`n finished · role ×k`** card per parent. It sits last
    in its group; ⏎ or a click unfolds it.
  - A finished agent still shows while a live or failed agent is below it (5d's `SHOWN`).

**Sizes (logical px):**

| Part | Value |
|---|---|
| Graph padding (inside the pane card) | 10 16 8 |
| Breadcrumb line | h22, 11.5 |
| Column header line | h24: 11.5 medium `dim`, count 6px later in `#4d4e52` |
| Column widths | orchestrators **236**, threads **300**, depth columns **272** |
| Gap between columns (holds the connectors) | **36**; the spine sits at its middle, 18 left of the child column |
| Card | h**54**, radius 8, bg `#1d1e21`, 1px `border`, padding 9 10 0 12, two lines with gap 6 |
| Card gap in a column | 10 |
| Fold card (`n finished`) | h40, transparent bg, status squares 12×12 (radius 3, gap 3), text 12 `dim` |
| Selected agent card | 68 + detail lines (17 each, +3 per item); width unchanged |
| Selected thread card | 64 + 20 per phase |
| Connector | 1.5px `input`; on the selected path `#ffffff4d` |
| Horizontal scrollbar | h6 track `#ffffff0a`, radius 3, thumb `#ffffff33`, 6 above the key line |
| Key line | h22, 11.5 `dim`, keys in `muted` 500 |

Columns **keep their width and align left**. The space left over on the right stays empty;
the operator decided this in turn 5, because a stretched card's sweep runs too long.

**What a card shows:**

- **Orchestrator:**
  - Line 1: harness icon 16, title 500, the aggregate marks (§3).
  - Line 2: `<harness> · <age> · tab <n>`.
  - Edge and moving bar as in §4.7. A finished orchestrator is drawn at 0.55 opacity.
- **Thread:**
  - Line 1: name 500, its one mark.
  - Line 2: phase bars (§4.5), then the current phase and its note (`CI · 3 failing`, `review
    r2`, `author · waits on Q14`).
  - **Selected thread:** border `accent`, and line 2 becomes `<phase> · <age> · N agents, L
    levels`. Then the phase list follows, one row per phase, h20, 12: a glyph (✓ done, ▸ now,
    ✗ failed, ? waiting, · next), the name, and the time on the right (`09:02–09:20`, `since
    09:43`). Done rows are `muted`, the current one `foreground`, next ones `dim`.
- **Agent:**
  - Line 1: harness icon 14, role (`muted`; `foreground` on the selected path), `id · model` in
    `dim` 11.5, then the status mark.
  - Line 2: the activity (`dim`, or `blocked` when it needs you). On the right goes `age · N
    calls`, or **`› n`** when its children's column is off-screen.
  - The needs-you edge is the card's left border on `fail/blk/wait` cards. The moving bar is a
    segment travelling along the bottom border of every `run`/`rev` card, as the TUI sweep does
    (§4.7 has the geometry).
- **Selected agent:** border `accent`, bg `#202226`. Line 1 ends with the status word
  (`needs answer`, `failed`, `running`, … in that status's colour) instead of the mark. Below
  it:
  - detail rows (12, line 17): `now <activity>`, then `? <question>` per open question
    (`blocked`, wrapped), then `✗ <error>`
  - an action row of h24 buttons, radius 6, 11.5: **Peek ⏎** (solid; **Open pane ⏎** for an
    agent with a pane), **Transcript t**, **Answer a** (**Allow / deny a** when blocked;
    `dim` when there's nothing to answer)
- **Selected path:** every card from the orchestrator down to the selection gets a brighter
  border (`#ffffff38`), and its connector stubs light up (`#ffffff4d`).

### 6.5 Horizontal scrolling (the window onto the columns)

- **Window rule** (5d's, unchanged):
  1. Start with the focused column, the column before it (its parents) and, if it fits, the
     column after it (its children).
  2. Then add more columns while they fit: right first, then left.
  3. Never drop the focused column or the one before it.
- **Fit:** fit against the graph's inner width, about 1230 at a 320 sidebar and a 1600-wide
  window, minus 24.
- **Left inset:** when columns are hidden on the left, the first visible column starts 24 in,
  leaving room for the lines that come in from the left edge.
- **Focus moves the window.** In D the thread column has focus: orchestrators, threads and
  depths 1–2 show, and depths 3–5 are off to the right. In D2, after → → → → onto the
  checker at depth 4, the window is depths 2–5. Orchestrators, threads and depth 1 are off to
  the left.
- **What stays visible when columns are off-screen:**
  - **Breadcrumb line:** pinned above the column headers. Left: `‹ <orchestrator> › <thread> ›
    <agent id> …   N columns left`, naming the selected path through the hidden columns, in
    `muted`, with the count in `#4d4e52`. Right: `depth a–b · N columns right ›`. Both
    disappear when nothing is hidden on that side.
  - **Column headers:** pinned while the columns scroll vertically.
  - **`› n` on cards** whose children's column is hidden.
  - **Lines in from the left edge** when a group's parent column is hidden.
  - **26px fades** on a hidden side (`background` to transparent).
  - **The scrollbar thumb.** Left = the width of the hidden columns on the left; width = the
    shown span; both out of the total width of all columns.
- **Wheel:**
  - The wheel over a column scrolls that column vertically by 3 rows (each column scrolls on
    its own). A column that overflows shows a `↓ N more below` chip at its bottom: h20, radius
    5, bg `raised`, 11 `muted`. A wheel-scrolled column stops following the selection until
    the selection changes.
  - **shift+wheel, or a horizontal wheel or trackpad,** scrolls the window sideways without
    moving the selection. The next focus or selection change scrolls back to it.
  - `shift+wheel scroll` is in the key line while anything is off-screen.
- **Click safety (TUI rule):** after any scroll, a card that slid under a still pointer
  ignores clicks for 0.3 s. After a refresh that moved cards, clicks wait 1 s per card.

### 6.6 Keys and mouse

The Swarm tab's mod takes keys while it has focus:

| Key | Action |
|---|---|
| `↑` `↓` (`k` `j`) | Move within the focused column; in a depth column, across parents |
| `→` (`l`) | Into the next column. From a depth-k agent: into its children, picking the child last selected there, else the first one. From the thread: onto the depth-1 agent on the selected path. |
| `←` (`h`) | Back a column. From a spawned agent: up to the agent that spawned it. |
| `⏎` | Open: `pane.focus` for an agent with a pane (orchestrators, terminal sub-agents), Peek (§4.9) for a pane-less subagent, unfold on a `n finished` card |
| `t` | Transcript |
| `a` | Answer: herdr-inbox for an open question, allow/deny for a permission |
| `i` | Inbox |
| `g` | Go to the orchestrator's pane |

Mouse:

- A click on an **orchestrator or thread** card selects it and focuses its column (drill-down).
  Double-click or ⏎ opens the orchestrator's pane.
- A click on an **agent** card selects it and opens it, the same as clicking its sidebar row:
  `agent.focus {terminal}` / `pane.focus` when it has a pane, the Peek popup when it doesn't.
  So clicking never leaves you without a way back: Peek is a popup, and the Swarm tab stays
  first.
- Clicks on the action buttons run their action.

Clicking a node never types into a pane (panorama stays read-only).

### 6.7 What the mod needs from gpui-kit

- **The canvas:** cards and connectors are absolutely positioned, the way the HTML does it.
  This needs `relative()` / `absolute()` divs with `left` / `top` / `w` / `h`.
- **Horizontal scrolling:** the window isn't a native horizontal scroll. The mod computes which
  columns show (§6.5) and lays out only those, so arrow keys can drive it.
  - shift+wheel and horizontal wheel events have to reach the mod: `on_scroll_wheel` with
    `delta.x`, or shift held.
  - The scrollbar is drawn by the mod (two divs), not `overflow_x_scrollbar()`.
  - If gpui-kit can't deliver horizontal wheel deltas to a mod, keep keys and clicks only, and
    say so in the key line.
- **Vertical scrolling per column:** one `overflow_y_scrollbar()` container per visible column.
  But connectors cross between columns, so the simpler route is a per-column scroll offset
  the mod applies itself, with lines clipped to the canvas.
- **Text measurement** for wrapped question lines in the selected card. gpui wraps text
  itself; the card height can be `auto`, as long as the placement pass runs after layout, or
  is estimated (the HTML estimates 34 characters a line).
- **Timer:** the same 50 ms timer as the sidebar (§4.7), only while an edge or bar is on screen.

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
