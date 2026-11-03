# herdr-panorama as a herdr-app mod: plan

Status: **approved by the operator for implementation** (2026-11-03). Delivered with the
`conductor` workflow (lanes, authors, independent reviewers). Baseline: `origin/main`.

## 1. Goal

herdr-panorama gets a second flavour: a **herdr-app** mod set, beside the existing herdr TUI
plugin, which keeps working unchanged. In herdr-app, panorama replaces the built-in Agents
panel in the left sidebar, keeps (and slightly improves) the Spaces section, and adds a
per-space **graph tab**. It shows the same swarm the TUI shows: orchestrators → threads →
agents → their subagents, with panorama's statuses and animations, plus a harness icon on
every agent row.

The design is final and is the source of truth for the look:

- `docs/design/herdr-app/` — PNG per frame (`v2-a-window`, `v2-b-sidebar`, `v2-c-peek`,
  `v2-d-graph-tab`), the reference HTML (`Herdr App Panorama v2.dc.html` + `icons/`, `ui/`),
  and **`SPEC.md`**: theme() colour names, sizes, status marks, fold rules, animation timings,
  click behaviour, and the list of things a real mod has to do differently from the HTML.
- The operator's screenshot of stock herdr-app is the reference for everything that is not
  the sidebar or the graph tab: it must look unchanged.

## 2. Observable behaviour (acceptance)

1. **Sidebar (Agents slot).** A `panorama` mod replaces the `agents` block of the layout.
   - Orchestrators sit in herdr-app's own priority sections (Needs you → Done → Working →
     Idle), each placed by its most urgent descendant.
   - Rows: orchestrator → thread → agent → subagent → its subagent (nesting has no fixed limit;
     at least 4 levels render legibly at the sidebar width).
   - Status marks per SPEC: working spinner (the app's own `spinner.svg`, `.spin(900)`),
     reviewing `»`, waiting `?N`, blocked on permission `!`, failed `✗`, done `✓`/dot,
     queued/idle ring.
   - Animations: spinners; a moving bar under an orchestrator while its agents work; an eased,
     blinking 2px edge on rows that need the operator.
   - Default folding per SPEC (threads always visible; agents only on the path to something
     that needs the operator; finished agents folded into "✓ N done · roles"; chevrons fold).
   - **Harness icon on every agent row** (Claude Code, Codex, Gemini, Cursor, Amp, …) from the
     app's own icon set (`program.icon` for terminals; `icons/<harness>.svg` for pane-less
     subagents), full colour or tinted per `program.tint`.
   - **Parity floor:** every agent the stock `agents()` list shows also appears in panorama,
     even when panorama has no transcript data for it (then as a plain orchestrator row with
     no threads). The panel must never show fewer agents than the stock one.
2. **Clicks.** An orchestrator (has a terminal) → `agent.focus {terminal}` (same as stock).
   A pane-less subagent → the mod's own Peek popup (`openOverlay`, kind `popup`): status,
   error, last message, "Open orchestrator", "Transcript". Chevrons fold/unfold.
3. **Spaces.** A clone of the shipped `spaces` mod, unchanged apart from: a coral count of
   needs-you items and a spinner while something works in that space; the Spaces block sized
   to its content so the swarm gets the rest of the column.
   **Clicking a space** shows that space with its **panorama graph tab as the first tab,
   selected**: if the space has no graph tab, create it
   (`tab.new {space, name, mod: "panorama-graph", props: {space}}` then `tab.move` to index 1),
   then `tab.select` it. The tab is named "Swarm" (pill per SPEC). `tab.new` returns no id:
   find the new tab in `spaces()`; check whether `tab.move`'s index is 0- or 1-based; nothing
   pins the tab first, so re-move it only when the mod creates it, never fight a user's drag.
4. **Graph tab.** A `panorama-graph` mod tab showing that space's swarm as the graph in
   frame D (orchestrators → threads → agents → subagents, restyled to herdr-app). Clicking an
   agent node behaves exactly like clicking it in the sidebar (rule 2).
5. **Install / uninstall.** One command installs the mods and a `panorama` layout (a clone of
   `main` with the `agents` block → `panorama` and `spaces` → `panorama-spaces`) into
   `~/.config/herdr-app` without touching shipped or personal files, merges its keys into
   `keys.json` (asking before replacing a used key), and prints how to switch
   (`ctrl-shift-l` or `herdr-app do layout.switch '{"name":"panorama"}'`). Uninstall removes
   exactly what install added.
6. **TUI flavour untouched.** Everything that works in the herdr TUI plugin today still works;
   its tests stay green.

## 3. Architecture

```
Claude Code / Codex hooks ──▶ session ↔ terminal records (HERDR_PANE_ID or HC_TERMINAL_ID)
transcripts, events, inbox.db ─┐
hc ls --json / hc events ──────┼─▶ Rust collector ─▶ app feed v1 (JSON file, atomic writes)
herdr api snapshot (TUI) ──────┘                          │ watchFolder
                                                          ▼
                       herdr-app mods (JavaScript, gpui-kit): panorama (sidebar),
                       panorama-spaces, panorama-graph (tab), panorama-peek (overlay)
                       joined with agents() / panes() / spaces() / terminals() / theme()
```

### 3.1 Host seam (Rust)

- Detect the host: `HC_TERMINAL_ID` set → herdr-core (herdr-app); `HERDR_PANE_ID` set → herdr
  TUI. Both may be absent (snapshot run from a plain shell): then use what the records say.
- Hooks (`panorama hook`, `panorama codex-hook`) record the terminal address next to the pane
  id. herdr-core addresses look like `t2`; remote ones `vps/t7`.
- Collector source for herdr-core: `hc ls --json` (address, agent, cwd, state, status, turn,
  where) as the pane list and status source, instead of the herdr API snapshot. Keep this
  behind a trait so the TUI path is untouched. Prefer a seam that could later move into
  `herdr-agent-kit`; do not change the kit in this effort.
- The Codex integration (`docs/plans/codex-integration.md`, in progress on its own branch)
  owns Codex discovery; build on what has merged to `main` and don't duplicate it.

### 3.2 App feed v1 (the contract between Rust and the mods)

- Written first, as `docs/protocol/app-feed-v1.md`, with a fixture
  (`tests/fixtures/app-feed/*.json`) covering every status, 4+ nesting levels, mixed
  harnesses, a pane-less subagent with an error, inbox questions. The mod lane builds against
  the fixture, so it doesn't wait for the collector.
- Shape: the existing `snapshot --json` model, versioned (`"v": 1`), plus: `terminal`
  (address) and `host` per orchestrator, `harness` per agent, nothing herdr-app already knows
  (space/tab membership comes from the mod's join with `terminals()`/`panes()`).
- Delivery: `panorama app-feed --ensure` starts (once, detached, like the inbox worker) a
  watcher that re-collects on change and writes `~/.config/herdr-app/data/panorama/feed.json`
  atomically (write temp + rename), idling out when no app is running. The sidebar mod calls
  it on start via `exec` and reloads on `watchFolder`; a slow timer is the fallback.
- Feed errors are data, not crashes: `errors[]` in the feed, shown subtly by the mod.

### 3.3 Mods (JavaScript, gpui-kit) — repo directory `app/`

```
app/
  mods/panorama/          sidebar: main.js, tree.js, marks.js, feed.js, mod.json {"place":"none"}
  mods/panorama-spaces/   clone of shipped spaces + counts + click → graph tab
  mods/panorama-graph/    the graph tab (props.space)
  mods/panorama-peek/     the Peek popup
  layouts/panorama.json   clone of main with the two blocks swapped
  keys.json               the keys the mods expect
  RECIPE.md, preview.png  for sharing on herdr.dev (layout gallery)
```

- Read `~/.config/herdr-app/AGENTS.md`, `herdr.d.ts`, `gpui-kit.d.ts` (regenerated per app
  version: they are the truth) and the shipped `agents`, `spaces`, `workspace-chrome` mods
  before writing code; reuse their patterns (status sections, spinner, subRow tree lines,
  theme() colours).
- One feed reader: the sidebar mod owns the feed and broadcasts it with
  `setShared("panorama.feed", …)`; the graph and spaces mods read `shared()`, and read the file
  themselves only when the sidebar isn't in the layout.
- Animations with `cx.timer.every` + `.opacity()`; redraw only what animates and keep the tick
  modest (measure; SPEC notes the cost).
- Every mod passes `herdr-app check app/mods/<name>` (run it locally; add a script
  `scripts/app-check` that runs all of them). CI can't run herdr-app; keep JS logic that can
  be tested (tree building, folding, sorting, join) in plain modules with a small Node test
  run in CI against the fixture.

### 3.4 Install

`panorama app install|uninstall` (Rust subcommand or a script under `scripts/`) copies
`app/` into `~/.config/herdr-app` (never overwrites; refuses on name clashes), merges
`keys.json`, records what it installed for uninstall, then runs `herdr-app check` on each mod.
It never edits shipped files and never switches the operator's layout.

## 4. Lanes

Dependencies: A1 (contract + fixture) unblocks lane B. Lanes A and B then run in parallel;
C joins them.

| Lane | Slice | Contents | Done when |
|---|---|---|---|
| A feed | A1 | `docs/protocol/app-feed-v1.md` + fixtures | reviewed; fixture validates against the doc |
| A feed | A2 | host detection; hooks record `HC_TERMINAL_ID`; `hc` collector source behind a trait | unit tests with recorded `hc ls --json`; TUI tests green |
| A feed | A3 | per-agent harness; feed v1 serialisation; `panorama app-feed --ensure` writer | feed from a fixture world equals the fixture; atomic write test; idle-out test |
| B mods | B1 | `app/` scaffold, install/uninstall, `scripts/app-check`, Node test harness in CI | installs into a temp app home; uninstall leaves no trace |
| B mods | B2 | sidebar mod on the fixture: sections, tree, marks, icons, folds, animations | matches `v2-b-sidebar.png` per SPEC; `herdr-app check` passes |
| B mods | B3 | `panorama-spaces` (counts, spinner, click → graph tab) + `panorama-graph` tab | space click creates/selects the first tab; graph matches `v2-d-graph-tab.png` |
| B mods | B4 | Peek popup + click routing in sidebar and graph | rule 2 holds in both places |
| C integration | C1 | live feed end to end in a real herdr-app; parity floor; perf check of the tick | operator's visual check (below) |
| C integration | C2 | README section "herdr-app flavour", screenshots, RECIPE.md/preview.png | docs reviewed |

## 5. Verification

- Rust: `cargo test`, clippy, the existing parity/oracle tools stay green.
- JS: Node tests over the fixture for tree/fold/sort/join logic; `scripts/app-check` for every
  mod.
- Visual: screenshots of the installed mods compared with `docs/design/herdr-app/*.png`
  (herdr-app can't be screenshotted headlessly; capture the window, or ask the operator).
- **Personal-review hold:** before the integration branch merges to `main`, the operator looks
  at the installed panorama layout in their own herdr-app and approves it.

## 6. Authority and boundaries

- Implement on the integration branch `app/herdr-app-mod` (from `origin/main`). Each slice is a
  PR into it, reviewed by an independent reviewer; merge into the integration branch on green
  CI and a clean review.
- Merging the integration branch into `main` needs the operator's approval (the visual check
  above). No releases, tags or marketplace/gallery publishing.
- The operator's live herdr-app: installing into `~/.config/herdr-app` is allowed only through
  the installer, only with `panorama*` names and `mod.json` `"place": "none"` (so nothing docks
  into their current layout), never editing shipped or personal files, never switching their
  layout. Use a temporary app home for automated tests.
- Don't change the operator's Claude Code, Codex or herdr configuration, and don't touch the
  herdr TUI plugin installation.
- Operator questions: ask once, keep working with a stated assumption.

## 7. Out of scope (follow-ups)

- Answering inbox questions inside herdr-app (the inbox's own app port). Here the `?N` chips and
  Peek show the questions; answering stays in the TUI inbox for now.
- Moving the host seam into `herdr-agent-kit`.
- Replacing `workspace-status` so its counts match the sidebar.
- Remote machines (`vps/t7`) beyond showing what `hc ls` reports.
