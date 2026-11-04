# U8a sidebar and Peek on feed v2: author capture

Capture source: `4957331667bb7dbad3bb5a712534c23b83f31361` (review r1 fixes) (clean `app/` tree). Per-file SHA-256 of the captured mods is in [provenance.json](provenance.json).

[V11 B design / native](compare-b.png) · [V11 C design / native](compare-c.png) · [native sidebar, Test](native-b-sidebar.png) · [native sidebar, TrafficLens](native-b-trafficlens.png) · [native Peek](native-c-peek.png)

Each native image is the left 640×1430 physical pixels of a 1288-wide window at zoom 2.0, the scale of the design's B and C frames. It is cropped to the sidebar and leaves out the pane header, which shows the scratch path. There is no rescaling.

## How it was made

- A temporary `HERDR_APP_HOME`, `HOME` and XDG dirs, and a private `hc` core. The core was verified as separate, with 0 live terminals at start.
- Owned status-reporter terminals, one per design root, kept as tabs named with the root's title.
- The design's `SW` dataset from `state-refresh-v11/fixture.js` (V11 names, `kids` trees) was converted to a **feed v2** packet. Its root and agent templates come from `tests/fixtures/app-feed/swarm.json`.
- One extra `herdr-tui` root has no app agent, which exercises feed-only roots.
- Only seeding is wrapped:
  - The sidebar wrapper publishes the private packet.
  - For C it opens the real `panorama-peek` view on the design's `fixer` agent.
  - Peek's wrapper resolves that subject from the same packet, because the seeded install has no writer generation. Rendering is the shipped code.
- No personal content or operator state was used. `herdr-app status` reported `errors[]` empty. The app was terminated, the private core stopped and the scratch dir removed. The capture held `runtime.lock` throughout.

## Differences from the design, and why

- **`Herdr multi-account` shows Idle under Idle**, not Done under Done. As in the V10 capture, the app reports a looked-at `done` agent as `idle`. Its line 2 `✓2` is the per-direct-subagent finished count, as designed.
- **`Nightly dependency bumps` reads Stale** (not in the design's data). It is a root that has no app agent, such as a TUI-hosted root. These are now listed in the ordinary sections under their feed title (issue #161, feed v2 "Identity, tree and app joins"). They have no tab number, click target or space, and they stay out of the header totals. The seeded packet has no live writer, so the feed is stale. A feed-only root then shows as Stale, with no needs, tiles or counts, rather than as fresh evidence.
- **No hover** on entry 2: nothing drove synthetic pointer input.
- **Running glyphs** are captured mid-rotation. The header `◌` is the existing stale indicator: the seeded packet has no live writer.
- **Peek position** follows the private window's pointer, not the design's anchor next to the row. The Transcript button is disabled because the seed has no transcript path.
- **Font weights:** the isolated host draws every weight the same, as noted in the V10 capture.

## Exercised

- The docked card holds its list:
  - layer 1 (`Fix PR #142 fold test` with `+1✓` for its finished child, `Review PR #144 r2`, `Write PR #145 retry banner`);
  - layer 2 (`Explore icon call sites`, whose run shows its running descendant);
  - the `3 finished · titles` fold.
- Line 2 of a non-docked card has one tile per active direct subagent, then `✓n` (TrafficLens: `! ↻ ○ ✓2`).
- Root titles come from the operator-named tab (D3, D6). The space name is not the title.
- Peek is titled by the agent's name, with `role · model`, the V11 state badge, error, last message and `Open root pane`.

Not shown: fold toggles, `+N older` (covered by the Node tests on the v2 fixture) and the graph, which is U8b.
