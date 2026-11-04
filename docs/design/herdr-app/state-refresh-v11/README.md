# Panorama V11: one uniform agent tree, agents named by their spawner (design proposal)

**These are synthetic design renders, not installed screenshots.** Nothing in the product, runtime or configuration changes. V3, V4 and V5 stay in Git unedited.

The operator's feedback on V5: *"the graph looks good, but the sidebar is quite far from the TUI; rework it into how it looks there."* The references are the current TUI dock, as rendered by the Rust app (`docs/screenshots/dock-app.png`, `deep-5d-dock-app.png`), and the approved "Swarm View 10 focus" design. Its local source is `docs/design/swarm-focus.js` (turn 10: the docked 9e panel). The code that draws it is `crates/panorama-core/src/view/dock/{row,items,tiles,state}.rs`.

**Unchanged from V5:** the graph (D/D2), Peek, tabs and spaces.

## The sidebar, rule by rule from the TUI

| TUI dock | V6 |
|---|---|
| Header `SWARM 5 orchestrators … ?3 !1 ✗1` | The host's `Agents` header with the same totals on the right: `?N` and `!N` in amber, `✗N` in red. |
| A flat numbered list, no priority sections | Same. The number is the dock position or tab, dim. |
| Line 1: number, **state tile**, name; right side **`?N` + state word** | Number, 18px tile, harness icon, name; right side `?N` (amber, bold) and the word (`failed` / `blocked` / `working` / `needs you` / `waiting` / `done` / `idle`) in its hue. Done and idle words are dim. |
| Line 2: **one tile per active thread** (its most urgent agent), `✓n` for done threads, then `N active · M agents` | Same tile strip at 16px, then `space · N active · M agents`. The space is added because the app sidebar spans spaces. |
| Entry state: done > failed > blocked > working > needs you > waiting > idle | Same (`dock_state`). |
| The **docked** entry: selected fill `#1E2937` on its two lines, key-coloured bar `#E2CF9C` down its whole height, bold name; line 2 shows counts only | Same. |
| Its list: **thread rows** (tile, name, own agents' glyphs on the right) and **spawning-agent rows** (tree branch, bare coloured glyph, role, children's glyphs); `N done` row. Leaf agents get no rows. | Same (`items_of`, `draw_thread`, `draw_spawn`, `agent_glyphs`: most urgent first, then `+N✓`). |
| Hover: `›` in place of the number, a step-lighter fill | Same (shown on entry 2 in B). |
| `+ New agent  n` | Same. |
| Running glyphs spin, needs-you tiles blink | The running arc turns; tiles that need you breathe 70–100%. Exports are frozen. |

Kept from V5: square 18/16px tiles with SVG glyphs. TUI tiles are 3 cells wide only because of the monospace grid, and the operator disliked stretched icons. Hues and fills are `palette.rs`. Inter is used in the exports.

Clicks are unchanged: an entry focuses its orchestrator's pane, which makes it the docked entry. A thread row or spawning-agent row opens or focuses its pane, or opens Peek (C shows Peek on `fixer`). `?N` navigates to answer.

## V11 changes, from the operator's decisions of 2026-11-04

> "Graph should just treat all agents and their subagents like a graph and different layers should not have any special names or components"
> "Can we just ask each agent to name itself when starting? Then no threads or anything. And names would come from agents of any level, right?"
> Root sessions: "use 1 and let 3 override it. 2 I don't think we really need."

- **No orchestrators or threads.** A root session's children are its own subagents, and theirs follow at any depth. Threads, phases, phase pips and the thread card are gone.
- **Names.**
  - A subagent's title is the name its spawner gave it: Claude's Agent/Task `description`, Codex's task name. It applies at every level, with no agent cooperation needed.
  - A root's title is the harness session title, overridden by the operator's herdr tab/pane name. There's no `set_name` tool.
  - Role, id and model are secondary: they appear in the expanded card's `agent` line, and the model appears on line 2.
- **Graph (D/D2):**
  - Columns are `Depth 0, 1, 2…`; depth 0 is the space's root sessions.
  - **One card component at every depth.** Line 1 is the harness icon, the name, then aggregate need tiles (✗ / ! / ?N, when the subtree needs you) and the own state badge. Line 2 is the current activity, then `model · age` (`tab N · age` for a root, whose click focuses its pane).
  - Finished siblings fold into `N finished · names…`. The window, breadcrumb, horizontal scroll and click split are unchanged, and the breadcrumb is made of names.
- **Sidebar dock:**
  - The root card is unchanged from V10, except line 2 now has one tile per active **direct subagent** (its subtree's most urgent state), then `✓n` and the counts.
  - The docked list uses **one row component for every agent**: own state glyph, name, and a run of states.
    - Layer 1 is the root's subagents and layer 2 is theirs; finished siblings fold into a `N finished · names` row.
    - A layer-1 row's run counts only its finished children (`+N✓`), because its live children are listed under it.
    - A layer-2 row's run shows **all** its descendants (up to 6, then `+N✓`).
  - Tree lines are computed as in V7–V10.
- **Peek** is titled with the agent's name, with `role · model` beside it.
- Styling (V3 look, V5 tiles and glyphs, V8 graph-style cards, V10 sections, active space and Spaces tiles) is unchanged.
- **Out of scope here:** the core-model, TUI and app-feed migration. It's covered by the separate plan `docs/plans/uniform-agent-tree.md` (branch `plan/uniform-agent-tree`).

## V10 changes, from the operator's review of V9

- **Priority sections stay.** Needs you / Done / Working / Idle, from the approved v2 spec, group the cards again. V6–V9 had dropped them to match the TUI dock, which has no sections. That was a misreading.
- **The active space only.** The Agents list shows the orchestrators of the active space. Each card's number is its tab position in that space (`Test`: 1 Panorama, 2 Herdr multi-account), matching the graph's `tab N`. Line 2 drops the now-redundant space name. Header totals cover the active space, and zero totals are hidden.
- **Spaces carry their state.** Each Spaces row ends with one tile per need kind (`✗`, `!`, `?N` with the supplied count), then a running tile. This uses the same vocabulary as the cards, replacing v2's single count chip. Switching spaces switches the list.

## V9 changes, from the operator's review of V8 ("limit to 2 layers of subagents, lower the gaps, show deeper elements as a horizontal list of states")

- **Depth limit (two layers: the thread, then its agents).** The docked list draws thread rows (layer 1) and the agents directly under them (layer 2), using `MAXL = 2` in `fixture.js`. Nothing deeper gets its own row. A layer-2 row with deeper descendants shows **all** of their states in its right-hand run, most urgent first, up to 6, then `+N✓`. In A, `explorer` and its researcher fold into `reviewer`'s run. In D2, `reviewer v1` carries `✗ ? ↻ ↻ ↻ ↻ +6✓` for its whole subtree.
- **No duplicate state on the card.** Line 1 no longer has a left state tile. The filled badge on the right (`× Failed`, `↻ Running`…) is the card's state, and a 16px harness icon leads the card. The tree trunk starts from the icon's centre. Thread and agent rows inside keep their own tiles and glyphs.
- **Denser state runs.** Glyphs in a row's right-hand run sit 2px apart (were 5–9px), with `+N✓` 4px after them. The thread tile strip on each card's second line also uses 2px gaps.
- **Denser.**
  - Tree rows: 20px (were 24), with 14px thread tiles, 11px glyphs and an 18px indent per level.
  - Card headers: 25 + 19px (were 27 + 21).
  - Gaps: 4px between cards (were 6).
  - The tree-line geometry is recomputed for the new sizes. Each vertical still runs through its parent's lead centre.
- **Fixed:** spawn rows had used the class `sp`, which collides with base.css Spaces rows (`gap:9px`). It's renamed `spw`, so glyph runs are tight again. V6–V8 renders show the wider spacing that bug caused.

## V8 changes, from the operator's review of V7 ("make the sidebar cards look more like the graph's, with colours by state")

- Each orchestrator's container is drawn exactly like a graph card (`proposal.css` `.dn`). The body is the state hue at about 5.5% over `#17191D`, the 1px border is the state hue at about 22%, and a 3px left edge is the full state hue. It's flat and native: no gradient or shadow is needed in gpui, and `mix()` gives the composited fill.
- The right side of line 1 now shows the graph's filled state badge (`× Failed`, `! Permission`, `↻ Running`, `✓ Done`, `○ Idle`) instead of the plain word. When the entry also has questions, a compact `?N` tile sits before the badge. A needs-answer entry shows `?N Answer`.
- The docked entry takes the graph's selection treatment: a pale `#B4C8E4` border and a lifted base, while keeping its state tint, edge and badge. This replaces the TUI's yellow bar and outline, so the sidebar and graph mark selection the same way.
- These stay from V7: one container per orchestrator with 6px gaps, the computed tree lines, the TUI dock structure (number, tile, name; thread tile strip and counts; the docked list of thread and spawn rows with glyph runs) and the header totals.

## V7 changes, from the operator's review of V6

- **One container per orchestrator.** Each entry is a rounded box: a neutral `#14161A` surface with a 1px `#FFFFFF12` border and 6px between boxes. A 2.5px inset bar in the entry's state hue runs down the inside left edge, so it's clear where each orchestrator starts and ends. The docked entry keeps the selected fill across the whole box, with a key-coloured outline (`#E2CF9C66`). Hover lightens the box.
- **Tree lines computed, not stitched.** V6 reused the 26px-row connector pieces in 24px rows, and their x positions didn't match the leads, so lines broke and missed their parents. V7 places each line from one geometry (`treeLines` in `fixture.js`): the header tile at x=28, level-L leads at 28+20·L, and every vertical through its parent's lead centre. The rules:
  - an elbow (`├` or a rounded `└`) leaves the parent's vertical at the row centre and stops 3px before the child;
  - a vertical continues only while that parent has later children;
  - a parent with children drops a line from the bottom of its own lead;
  - the trunk starts under the header tile and passes left of the counts.

  D2 shows it four levels deep (reviewer → researcher → researcher → checker, plus batch).

## Images

| File | |
|---|---|
| [compare-sidebar.png](compare-sidebar.png) | **TUI dock today** / V10 / V11, scaled to the same height |
| [v11-a-window.png](v11-a-window.png) | A, 1920×1080 |
| [v11-b-sidebar.png](v11-b-sidebar.png) | B, the sidebar at 2× with hover and legend |
| [v11-c-peek.png](v11-c-peek.png) | C, Peek from the dock's `fixer` row |
| [v11-d-graph-tab.png](v11-d-graph-tab.png) / [v11-d2-graph-scrolled.png](v11-d2-graph-scrolled.png) | D / D2, the graph (V5) with the V6 sidebar |
| [compare-graph.png](compare-graph.png), [compare-graph-deep.png](compare-graph-deep.png), [state-gallery.png](state-gallery.png) | as in V5 |

## Edit / reproduce

- `v7.css` adds the containers and tree lines on top of `v6.css` (the dock), which sits on top of `proposal.css` (V3) and `v5.css`, both unchanged.
- In `fixture.js`, the `dock()` builder (search `V6 sidebar`) mirrors the Rust functions named above.
- In `preview.html`, the dock row template appears twice: once for the window frames, once for B.

```sh
python3 export.py --scratch /a/private/scratch --fonts /dir/with/Inter-{Regular,Medium,SemiBold}.ttf
```

## Open questions

1. The approved v2 spec grouped orchestrators into Needs you / Done / Working / Idle sections. The TUI doesn't, and V6 follows the TUI. Confirm that the sections go away.
2. The TUI numbers entries by tab. With several spaces, tab numbers repeat, so V6 shows the list position. Alternatively it could keep tab numbers with the space as a prefix.
