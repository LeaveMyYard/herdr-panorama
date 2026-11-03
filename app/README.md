# herdr-app flavour

The panorama sidebar reads the v1 app feed, preserves every native stock agent,
and adds transcript trees, status marks, folding and harness icons. The spaces,
graph and Peek mods remain native scaffolds for their dedicated UI slices.
Installing does not select the panorama layout.
The TUI plugin is independent and is not changed by this installer.

The sidebar measures its viewport at the current app zoom and mounts only
intersecting rows, so offscreen animation timers pause. Its thumb is a native
vertical Slider labelled Agents, with an owned focus wrapper supporting arrows,
Home/End and PageUp/PageDown. It retains Slider accessibility semantics (value
1 at the top, 0 at the bottom); it does not announce a native scrollbar role.
The native scrollbar's pinned geometry, color and physical dimensions are
adapted with the Apache-2.0 attribution in NOTICE.

From a reviewed checkout, with an existing, canonical absolute app-home path:

```sh
scripts/app-install install --app-home /absolute/path/to/herdr-app
scripts/app-install uninstall --app-home /absolute/path/to/herdr-app
```

Linux or macOS, Python 3.11+ and native `herdr-app` are required. The installer
checks private copies of the planned mod bytes, using a temporary app home,
unique HC_SESSION,
and scratch XDG config/state/runtime paths with inherited HERDR_/HC_ endpoints
removed. Only the checker-generated gpui-kit.d.ts and jsconfig.json are copied
as explicitly fingerprinted optional static files; changed copies are preserved.
No generated SDK types are committed to Git. Install never calls layout.switch,
layout.clone or the TUI installer. If a native checker started an empty core,
cleanup verifies its private endpoint, executable, session/config environment and
zero children, then calls only that exact isolated session's documented core
stop. Captured process exit is checked without a post-stop status call (status
would autostart a new core). Uncertain cleanup preserves private diagnostic state.
After installation it prints the command to switch when the person chooses.
The sidebar reads app-home data/panorama/feed.json and invokes only the reviewed
app-private panorama executable. The remaining empty scaffolds do no I/O.

An optional explicit reviewed executable is copied privately:

```sh
scripts/app-install install --app-home /absolute/path/to/herdr-app \
  --binary /absolute/path/to/reviewed/panorama
```

The destination is `data/panorama/bin/panorama`, mode 0755. There is no PATH lookup
or global installation for panorama. The sidebar invokes the private app-feed capability probe and ensure command
with explicit app home and the initially pinned --install-id generation. Every
private invocation re-reads the receipt and rehashes that exact artifact; missing
or changed receipt permanently stops the old client. A missing artifact is degraded data; stock agents remain.

The installer refuses existing panorama mod names, layout/file destinations,
symlinks in app-home paths, hardlinked destination files and all binding
collisions, even identical actions. It never renames a collision silently.
`app/keys.json` is empty until a design specifies bindings. Nonempty merges are
covered with explicit test packages, including collision refusal and exact raw
original key-byte/mode restoration.

Operations lock the app-home directory itself, leaving no lock-file residue.
New files use atomic create-only rename (Linux renameat2 / macOS renameatx_np)
through stdlib ctypes, with no overwrite or link/unlink fallback.
A mode-0600 v2 data/panorama/install.json records a fresh installation ID,
file hashes/modes, created directories and key additions before payload writes.
Native checks run before publication; later failures roll back unchanged files,
including writes whose directory fsync failed after publication. Incomplete rollback retains its ownership journal for recovery.
After an interrupted published install, run uninstall before reinstalling. An
interruption before the journal can leave private temporary files/directories;
existing files remain unchanged in that interval.
Filesystem power-loss guarantees are outside this installer.

Uninstall validates its bounded package inventory before deleting anything.
Key-removal authority comes from this checkout's package key definitions, not
from a self-consistent manifest delta; use the same reviewed package for uninstall.
Modified files, permission changes, added files and changed bindings survive;
concrete leftovers are printed. For later key edits, only unchanged bindings
added by this install are removed. Malformed edited keys are preserved whole.
A substituted symlink or unsafe manifest refuses uninstall without following it;
resolve that explicit conflict before retrying. A current layout that references
removed mods is the person's responsibility; uninstall never switches layouts.

For v2, uninstall disables the generation, waits at most two seconds for its
cooperative lifetime lock, and validates exact runtime ownership before cleanup.
Before deleting static files it waits up to two more seconds for the current
frontend to observe its disabled generation and clear its owned shared state.
The installer reads shared.json strictly with an 8MiB bound and never edits it.
A crashed cached frontend refuses: manually reopen the current mod to clear its
disabled-generation cache, then retry. There is no automatic app launch/layout
switch. After static removal a final host-key check precedes key/runtime cleanup;
a late publication restores only unchanged static bytes removed by that attempt,
using exclusive creation. Concurrent personal files or restore errors remain
preserved and reported as recovery gaps; receipt/control remain for repair.
Committed/pending feed digests authorize only their recorded bytes. Unknown or
changed runtime files, staging or a busy timeout retain the receipt and refuse.
No PID or process-group signal is sent. An edited private binary is never run:
proven runtime can be cleaned using its original receipt, then the static binary
is preserved. Legacy v1 receipts support only their frozen static inventory.
The full contract is [app-feed runtime ownership](../docs/protocol/app-feed-runtime-v1.md).

The sidebar uses only the host-declared crypto/createHash and fs/promises
adapters for pre-exec fingerprinting; there is no Node runtime or package
dependency. The secondary fingerprinting guard accepts at most 256 MiB after reading.
The tested native host additionally refuses fs.readFile above 64 MiB; the
merged release-profile artifact is about 10 MiB. An oversized debug artifact
stays degraded and never executes through the frontend; stock agents remain. The host exposes
no pre-read bound, stat or no-follow operation here; the executable performs its
own receipt/path/mode checks under the lifecycle lock. Prehash does not close a
concurrent replacement race. Native filesystem grants permit the default
HOME-based app home; outside granted roots the sidebar degrades with stock
parity. Automated graphical app homes therefore live in owned HOME scratch.
Changed-age state is view-local. A sidebar, or a consumer in a layout without
any panorama reference, coordinates one file reader via an advisory lease. The
lease grants no writer or installation authority. Each live view retains its
matching-generation stale cache. Final close or missing/changed receipt clears
shared feed/owner to null only with matching generation and current owner token;
a losing old owner cannot clear another client. Native probes show null keys
remain persisted: the operator explicitly approved ONLY these two inert null slots after uninstall.
No cached feed, changed-age map, or other Panorama payload residue is approved.
Uninstall never edits global shared.json or arbitrary local stores.

The exact v2 empty data directories for panorama-spaces, panorama-graph and
panorama-peek are reserved exclusively and journaled before module load. Existing
directories cause refusal; uninstall removes only empty owned directories.
Personal added files are preserved and reported.

```sh
scripts/app-build --check
node --test app/tests/*.test.mjs
scripts/app-check
```

Node 22+ runs the A1 fixture harness and installer failure/collision/rollback,
interruption, manifest, symlink, concurrent operation and personal-file tests.
The separate App CI workflow executes those tests on Linux and macOS for every
PR base. Python syntax is checked without bytecode files. Tests use temporary
app homes and a checker fixture; `--package` and `--checker` select explicit test
packages/checkers. Omit both for a real installation. No test installs live.

`scripts/app-check` runs the actual native checker over every runtime mod with
isolated environment/state. herdr-app is not available in CI, so this is a local
gate, also exercised by a real install/uninstall into a temporary app home.
During conductor work, native checks run under the shared `runtime.lock`.
A native check covers initial/eager rendering only; it does not validate layout,
later async states, interactions, screenshots or visual design acceptance.

The layout derives from Herdr app 0.1.12's main layout with agents/spaces mod
references replaced, a 320px sidebar and a 143px Spaces block. Its Apache-2.0
license and provenance travel inside mods/panorama/. No artwork is copied; shipped mod/asset references stay intact.
