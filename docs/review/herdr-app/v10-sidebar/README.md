# V10 sidebar and Spaces: author capture

Capture source: `496c7ee550c3b57409dc49f1fd75487248e1008d` (review r1 fixes) (clean tree; see [provenance.json](provenance.json) for per-file SHA256).

[Design left / native centre and right](compare-sidebar.png) · [native, Test space](native-sidebar-test.png) · [native, TrafficLens selected](native-sidebar-trafficlens.png)

Each panel is the top 385×700 physical pixels of a 1920-wide window at zoom 1.2 (the
design's A frame scale), cropped to the sidebar. No rescaling.

How it was made: a temporary `HERDR_APP_HOME`, `HOME` and XDG dirs, a private `hc`
core (verified separate, 0 live terminals at start), owned status-reporter
terminals, and the design's `SW` dataset from `state-refresh-v10/fixture.js`
seeded as the feed. No personal content and no operator state. The crop leaves
out the pane header, which shows the scratch path.

## Differences from the design, and why

- **The tree under the docked card** is today's tree, drawn below the card. The
  V10 dock tree was PR3b, which has been cancelled in favour of V11.
- **`Herdr multi-account` shows Idle under Idle**, not Done under Done. The
  app reports an unseen `done` agent as `idle` once it has been looked at, and
  the card follows the existing priority sections, so the badge and section agree.
- **TrafficLens has no `✓2` tile.** The capture feed doesn't carry the design's
  synthetic `doneThreads` count.
- **TrafficLens header `!1`:** the orchestrator is blocked on its own agent's permission, and that counts once (review r1 M1). The capture before the fix showed `!2`.
- **Running glyphs are captured mid-rotation**, so some look like a `C`.
- **The header `◌`** is the existing stale-feed indicator. The seeded packet has
  no live writer.
- **Font weights:** the isolated host draws every font weight the same, so badge
  labels can't be checked against the design's Inter SemiBold here.

## Short manual smoke (review r1 L5)

The isolated capture run exercised these:
- `space.select` between Test and TrafficLens switches the list and its header totals.
- `agent.focus` makes a card docked, with the pale border.
- Tab-position numbers are 1 and 2 in both spaces.
- The Spaces tiles follow each space's needs.
- `herdr-app status` reported `errors[]` empty.

Not shown in images:
- **Hover:** `›`, the lighter fill and the pin button.
- **`+ New agent`:** clicking it, and the `n` key, which works only when the list has focus.
- **`✓N` tile on a non-docked card**, other than Herdr's `✓2`.

These were checked by reading the code against the design and by the native
`herdr-app check`; nothing drove synthetic mouse input. Coverage for them is in the
deferred issue.

Deferred automated coverage: see the issue linked from the PR.
