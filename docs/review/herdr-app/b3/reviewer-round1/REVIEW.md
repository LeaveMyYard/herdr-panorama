# Independent B3 review, round 1

Reviewed PR #148 at `583fac51c1bc6ad7cda011432efa0411b12f3ba3`, base `96ff7aea7d3725d2b77528b9f93b4c233adb01d6`. Verdict: **request changes**, B/H/M/L/N **0/0/1/0/0**. This evidence-only directory changes no runtime source.

## Medium: retain age in selected agent metadata

`app/mods/panorama-graph/cards.js:63` always renders only `id · model`. The selected branch removes the ordinary second-line age without adding it to the selected header. The approved D2 card shows `c1 · sonnet · 7m`, and the final reference HTML line 780 explicitly uses `id · model · age` for selected cards. The synthetic checker has a valid timestamp representing seven minutes, yet the independent native D2 capture omits it. Preserve the available age in selected metadata; do not invent an age when unavailable.

## Independent native comparison

Design is left; independently captured native app is right. Both panels are 1920×1080 at app zoom 1.2. The panels are concatenated without resizing or retouching. Raw native PNGs are included.

- [D](compare-d.png), [raw D](native-d.png).
- [D2](compare-d2.png), [raw D2](native-d2.png).

The reviewer executed the existing audited one-shot capture method in a separate task-owned temporary HOME, app home, HC_SESSION, XDG config/state/runtime and core. Before structural commands the core had zero terminals, its process environment matched the private paths, and sockets existed only in private scratch. Only existing compositor/display variables were reused. The captured app reported `errors: []`. Captured app/core processes exited and scratch was removed; the runtime lock was held only during each active stack.

The synthetic fixture comes from the approved SW2/T144/DEEP design source converted into the canonical v1 feed shape with exact live reporter-terminal joins. Wrapper subclasses seed data/selection only; production rendering/layout helpers remain unchanged. This home has no receipt: consumers correctly remain stale and private executable/navigation authority is unavailable. These captures prove native rendering of the seeded states, not fresh rich transcript integration.

Functional differences from static references: complete translated dataset has 24 agents rather than the reference's static 21; actual stock tab names/order, git tails, status acknowledgment and live root ages follow the isolated app. Unprovided phase times/notes are not fabricated. Fonts are the app's configured native fonts per SPEC §8. Answer is disabled without validated Inbox capability. Those differences do not explain the selected checker's missing available age.

## Separate real-artifact/current-generation smoke

`current-generation-manual.json` records an independent execution of the existing bounded manual recipe against the real optimized artifact SHA256 `da6d69e802a479d47aa154d3e6464c7286d0c06ef25304c9ca5a22e0aec328f2`; Rust source was verified identical to its `1269dac` source base. This run used a different isolated HOME/core from the synthetic capture.

The real writer published a native root's working→blocked change; frontend packets were fresh and pinned to the correct G. Active uninstall, cooperative writer exit, safe preference retention, reinstall under replacement G, fresh reinitialization and stale-G refusal passed. Both approved preference stores survived with exact bytes/modes; reused parents were absent from new receipt.dirs, graph store absent, personal fixture bytes/modes unchanged, and only the two approved null shared slots remained. Initial/reinstalled native PNGs are included. No actual Inbox open, answer, permission change, machine connection or natural rich transcript claim was made. All private stacks were stopped.

## Review checks

- 109 existing targeted tests: B3 action/graph/inbox/space/tab plus installer/runtime — all passed, no failures/skips.
- Canonical bundle closure, Rust formatting, diff whitespace, installer Python AST and app JSON parsing — passed.
- All six actual native mod checks — passed in private state under runtime.lock.
- Source review covered graph/spaces/tabs/header interactions/joins, read-only routing, finite six-mod inventory, explicitly frozen original v1 authority, prior-four-mod v2 refusal and bounded retained-store validation/reuse. B4 caller/viewer work remains separate.

No new automated feature cases or exhaustive input/performance matrix were added. Existing tests/evidence were retained; deferred coverage remains #143–146. Safety-critical fresh final review, exact-final-head CI/root readiness/merge and main installed visual approval remain outstanding.
