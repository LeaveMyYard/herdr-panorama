# Sidebar Agents scope: author capture

Capture source: `3b8d3feaabed4ede0996914a75074b8a90b0ea17` (clean `app/` tree). Per-file SHA-256 of the captured sidebar and Spaces mods is in [provenance.json](provenance.json).

[This space](this-space.png) · [All spaces](all-spaces.png)

Each image is the left 640×1430 physical pixels of an isolated 1288-wide window at zoom 2.0, cropped to the sidebar. There is no rescaling.

## How it was made

- The U8a recipe: a temporary `HERDR_APP_HOME`, `HOME` and XDG dirs, and a private `hc` core, verified separate with 0 live terminals at start. Owned status-reporter terminals stand in for the design's `SW` roots (`state-refresh-v11/fixture.js`), seeded as a feed v2 packet. Only the sidebar wrapper that publishes the packet is added; rendering is the shipped code.
- **This space** was shot first, with no `data/panorama-spaces/store.json` (`store_before: null`).
- Then the real write path ran: `herdr-app do panorama-spaces.agents-scope-all` (the command the scope chip sends) returned ok, and panorama-spaces wrote `{"agents.scope": "\"all\""}` to its store, mode 0600.
- The app was restarted on the same home. The sidebar read the stored scope at init, and **All spaces** was shot.
- `herdr-app status` reported `errors[]` empty. The app was terminated, the private core stopped and the scratch dir removed, under `runtime.lock`. No operator state or `~/.config/herdr-app` was touched.

## What it shows

- **This space**: the Test space's two roots only, header `?1 ×1`, line 2 unchanged from V11 (`3 active · 5 agents`).
- **All spaces**: the five roots of Test, TrafficLens and 65net in the same priority sections (Needs you 2, Working 1, Idle 2). Line 2 leads with `space · tab N` (V6's space on line 2), so the three `1`s read `Test · tab 1`, `TrafficLens · tab 1` and `65net · tab 1`. The header totals follow the scope: `?1 !1 ×1` adds TrafficLens's permission request.
- The Spaces rows are identical in both.

## Known limits

- The header `◌` is the existing stale indicator: the seeded packet has no live writer.
- No hover or pointer click was driven; the chip's click handler sends the same command the capture ran directly.
- Long line 2 text truncates at the card edge (`TrafficLens · tab 1 · 3 active · 3…`).
