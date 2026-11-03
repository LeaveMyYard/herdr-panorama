# Graph viewport motion evidence

These are actual `herdr-app shot` captures and process CPU/render-count samples from isolated synthetic app/core runs at source `c009ee27f0fb08de512719a326eb41e97cabca2d`. The runtime mod code is unchanged by this evidence-only commit. The tests used app zoom 1.0 and 1.2, independent native column offsets, and the measured canvas geometry. All retained successful-run native `status` results had `errors: []`.

| Zoom | Case | Graph redraws / 3.2 s | Motion timer after | App process CPU |
|---|---|---:|---|---:|
| 1.0 | below-viewport | 0 | False | 2.812% |
| 1.0 | below-scrolled-visible | 70 | True | 14.061% |
| 1.0 | below-scrolled-hidden | 0 | False | 2.812% |
| 1.0 | above-visible | 69 | True | 13.124% |
| 1.0 | above-scrolled-hidden | 0 | False | 2.5% |
| 1.0 | horizontal-hidden | 1 | False | 3.75% |
| 1.0 | horizontal-visible | 59 | True | 10.312% |
| 1.2 | below-viewport | 0 | False | 2.5% |
| 1.2 | below-scrolled-visible | 70 | True | 11.874% |
| 1.2 | below-scrolled-hidden | 0 | False | 2.812% |
| 1.2 | above-visible | 69 | True | 11.874% |
| 1.2 | above-scrolled-hidden | 0 | False | 2.5% |
| 1.2 | horizontal-hidden | 1 | False | 2.5% |
| 1.2 | horizontal-visible | 60 | True | 8.749% |

The above/below hidden cases have no visible animated edge or bottom strip. The horizontal-hidden case excludes the working depth-7 card from the computed column window. In each hidden case the graph's 50 ms motion timer is off; the single horizontal-case redraw is not a 20 fps loop. The graph still has its 30 s age/capability timer and receives ordinary native data/layout redraws. Visible cases demonstrate that the timer resumes when an actual animated border reenters the viewport. See each `motion-z*-perf.json` for measured dimensions, column windows, offsets, card coordinates, stable probe identity, active-window state, and source SHA; corresponding PNGs were visually inspected, including the actual last card after scrolling.

The temporary wrapper seeds fixture data and dispatches the product's actual `Graph.scroll` callback with paint intervals. It counts actual native View renders and reads its own app PID's CPU ticks; it does not emulate a hardware wheel device. Before each measured sample it resizes only the private layout's sidebar 320 -> 321 -> 320 and waits for layout to settle. This both exercises native remeasurement and establishes live frames. It does not run pointer/key automation, focus a desktop window, modify the compositor, or switch the operator's layout.

The fixture is deliberately synthetic and stale in the shared consumer: no installation receipt/private artifact is supplied, so the canonical client fails closed before executable dispatch. The missing-receipt tooltip visible in some captures is expected. No live feed collection or Inbox navigation occurred. These measurements prove graph viewport/timer behavior, not installer/writer lifecycle acceptance, fresh live feed integration, full app CPU performance, hardware input delivery, or final design fidelity.

Two failed development probes are retained separately. `pre-fix-zoom-finding-*` shows the original dirty development tree treating post-zoom size callback bounds as pre-zoom graph coordinates: at zoom 1.2 it used a height of 814 instead of about 678.33, claimed the working last card visible, and kept its timer active even though the PNG clipped it. This was a finding, not a pass; no exact clean source SHA is claimed for that dirty probe. Production now reads current `settings().zoom`, converts raw measurements on every render, and remeasures on native resize. `stalled-z1.0-*` is the first zoom-1 attempt, whose render counter remained at one and captured frame never followed the fixture/scroll controls. That attempt provides no visibility/performance proof. The successful retry verifies advancing frame counts and stable probe identity before accepting samples.

All stacks used the shared runtime lock only while running, distinct core identity/PID, zero initial live terminals, a task-owned temporary app home under `/home/leavemyyard/.cache`, separate HC_SESSION/XDG paths and scratch sockets, and only existing compositor environment values. Only the owned app process/core were stopped; temporary state was removed. The operator's app configuration, layout and terminals were preserved. The capture driver and raw status/log/isolation records remain in the task's durable `B/author-b3` scratch. Final frozen B3 gates and independent captures still follow the B2 merge and canonical regeneration.
