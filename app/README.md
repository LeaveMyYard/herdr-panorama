# herdr-app flavour

The panorama sidebar reads the v1 app feed, preserves every native stock agent,
and adds transcript trees, status marks, folding and harness icons. Spaces adds
per-space activity and needs counts while keeping stock machine/worktree/git
behavior. Selecting an expanded Space creates Swarm once at the first tab
position or selects its existing graph. The owned tabs and pane-header clones
add the Swarm glyph/count and graph header while retaining stock terminal behavior.
The tabs clone keeps the agent-usage-tabs Usage toggle for the layout's `usage`
panel, shown only while the separately installed `agent-usage` mod exists.
The graph uses fixed depth columns, independent vertical scrolling, folding,
keyboard navigation and selected-card details. A selected transcript thread
shows its recorded spawn tree; a root without a transcript thread can show its
proven same-space native-terminal tree. Pane-less detail rows open Peek with supplied
status, error and last message; Enter opens a verified live owning orchestrator.
Transcript (t) opens a readonly native viewer using the current pinned installation
and feed scope. It reads supplied absolute producer-local paths through existing
local line paging, preferring the agent transcript then the root transcript.
Arrows page and t refreshes; one read is pending at a time. Path/authority loss
clears displayed text. A same-path read error preserves the previous valid page
with the raw error. Oversized or escape-expanded responses remain unavailable
with their raw API error; pages are not an immutable whole-file snapshot.
Caller-owned overlay tokens keep the row highlighted until its own view closes;
Transcript replaces that view without clearing the replacement highlight.
Installing does not select the panorama layout.
The TUI plugin is independent and is not changed by this installer.

## Install and select

Use an optimized Rust `panorama` binary from the reviewed checkout, Linux or macOS,
Python 3.11+ and native herdr-app/herdr-core. The app home must already exist at a
canonical absolute path. From the checkout:

```sh
scripts/app-install install --app-home "$HOME/.config/herdr-app" \
  --binary /absolute/path/to/reviewed/panorama
```

Choose Panorama yourself with `ctrl-shift-l`, or:

```sh
herdr-app do layout.switch '{"name":"panorama"}'
```

For a custom app home, use the same `HERDR_APP_HOME` for app commands and pass its
path to the installer. Installation does not select a layout, configure session
hooks, change the TUI installation or add global key bindings. Name/key collisions
are refused. An install without the private binary keeps basic stock rows but
cannot start transcript enrichment; oversized debug builds may also be unavailable.

## Use it

- Click a space to create/select its Swarm tab; later clicks respect a tab you
  dragged elsewhere. Arrows or `hjkl` navigate the graph. Wheel scrolls a column;
  shift-wheel moves the column window.
- In the graph, a root/thread click selects and drills down; Enter or double-click
  opens its orchestrator. Clicking an agent opens its verified terminal or Peek.
  Sidebar roots focus their terminal; chevrons fold. `g` opens the orchestrator.
- Peek shows supplied status/error/last message. Transcript (`t`) opens a native
  read-only JSONL viewer; its arrows page 100 lines and `t` refreshes. Escape closes
  the popup/dialog. `a`/`i` navigate to an available existing Inbox bridge or verified
  permission pane; they never answer, type or allow/deny from the mod.

## Session coverage

Rich Claude/Codex data requires an exact current core-terminal/session association
and a supported local transcript/history source. The installer does not enable or
configure hooks or bind old sessions. Unverified sessions remain basic native roots;
a matching folder/title does not establish ownership. Claude core-session proof
comes from the separately configured opt-in `panorama hook session-start` running
under that session's actual Claude process. Existing local Codex sessions can be
bound explicitly when current ownership can be verified:

```sh
/absolute/app/home/data/panorama/bin/panorama codex-bind THREAD_ID --terminal tN
```

Replace both identities with the actual local session/terminal. This verifies root
metadata and the terminal's current foreground process; it is your chosen association,
not detection of which conversation a shared-daemon TUI displays. Rebind after an
in-process root switch. See [feed identity rules](../docs/protocol/app-feed-v1.md)
and [Codex setup](../docs/codex.md). Synthetic previews do not prove natural rich
session ingestion.

## Uninstall and troubleshoot

Use the reviewed checkout/package that installed the inventory:

```sh
scripts/app-install uninstall --app-home "$HOME/.config/herdr-app"
```

Choose another layout yourself before removing one you are using; uninstall never
switches it. Personal edits survive. The exact two null shared slots, two retained
preference stores and host layout-state snapshot are described below; the installer
does not rewrite personal shared state or acquire deletion ownership over retained
preference/layout files.

- **No threads:** verify the private release artifact and current session proof;
  older unbound roots are expected to stay basic.
- **Stale/unavailable feed:** inspect `herdr-app status` for the intended app home.
  Stock roots and last valid stale data remain; a visible cache is not live proof.
- **Frontend stop timeout:** manually reopen the current mod/layout so it can clear
  its disabled generation, then retry. The installer does not launch/select it.
- **Collision, unsafe retained store or runtime/receipt refusal:** preserve the
  reported files and resolve that conflict. Do not delete receipt/runtime/shared
  state to bypass checks. Older inventories need their original uninstall package.

## Runtime, ownership and verification details

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
layout.clone or the TUI installer. Native checking establishes and pins one verified empty private core before
loading modules. It verifies private HOME/app/XDG/session identity, uses scoped
shutdown, and observes the captured process exit before removing scratch state.
Uncertain identity or shutdown preserves diagnostics instead of deleting them.
After installation it prints the command to switch when the person chooses.
The sidebar and owned consumers read app-home data/panorama/feed.json through
the same generation-pinned client and invoke only the reviewed app-private
panorama executable. Graph question/Inbox actions navigate through its installed
TUI bridge when available; permission actions focus a verified owning terminal.
They never answer, type or change permissions/configuration.

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
matching-generation stale cache. Collections older than 15 seconds, missing a
finite timestamp, or timestamped in the future are stale even while the advisory
lease is renewed. Only the current owner can request one pending generation-
guarded recovery; ensure success alone does not freshen old collection bytes.
Final close or missing/changed receipt clears
shared feed/owner to null only with matching generation and current owner token;
a losing old owner cannot clear another client. Native probes show null keys
remain persisted: the operator explicitly approved ONLY these two inert null slots after uninstall.
No cached feed, transcript or changed-age map is retained. Uninstall never edits
global shared.json or arbitrary local stores.

The five finite mod-data directories are panorama-spaces, panorama-graph,
panorama-peek, panorama-tabs and panorama-pane under data/. New directories are
journaled before module load and removed only when empty. The operator also
approved retaining exactly data/panorama-spaces/store.json and
data/panorama-tabs/store.json plus their necessary parents: these preserve stock
machine folding/reconnection recovery and panel selection across restart and
reinstall. Those exact parents may preexist without entering the new receipt's
created-directory list or granting deletion authority. A known store must be a
no-follow regular single-link mode-0600 file, at most 1 MiB, containing a strict
UTF-8 JSON string-map with supported consumed preference shapes. Unsafe known
stores refuse before mutation. Unknown keys/files remain untouched; preferences
are never rewritten, chmodded or deleted by the installer. Personal added files
are preserved and reported.

The operator also approved **“Keep host layout state (Recommended)”** for exactly
`layouts/.state/panorama.json` and its necessary parent. Herdr may persist the full
Panorama layout snapshot there, including the automatic content-sized Spaces
`layout.resize` state. This is host-owned state preserved after uninstall; no
installer deletion, rewrite, chmod or adoption authority is granted. This exception
does not authorize another layout-state file, feed/transcript cache, changed-age
map, graph store or arbitrary host-state cleanup.

An older four-mod B2 v2 receipt is refused before mutation. Uninstall it using
the reviewed package that created it, then install this six-mod package. There
is no in-place receipt upgrade. The frozen original four-mod v1 inventory,
optional private binary/modes and original key authority remain supported.
Retained-preference/reinstall automation is deferred to [#145](https://github.com/LeaveMyYard/herdr-panorama/issues/145);
extended native input/visual coverage is tracked in [#146](https://github.com/LeaveMyYard/herdr-panorama/issues/146).

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
packages/checkers. Standalone compiler fixtures must explicitly supply
`--checker ABSOLUTE_PATH --checker-core none`. This opt-in contract is only for
trusted custom checkers that never start a core; it is not native runtime proof.
Normal installs and actual scripts/app-check gates use default managed mode.
The none mode still refuses uncertain endpoints without stopping an unproven
process, and never calls autostarting status without a private endpoint.
Omit custom fixture flags for a real installation. No test installs live.

`scripts/app-check` runs the actual native checker over every runtime mod with
isolated environment/state. herdr-app is not available in CI, so this is a local
gate, also exercised by a real install/uninstall into a temporary app home.
During conductor work, native checks run under the shared `runtime.lock`.
A native check covers initial/eager rendering only; it does not validate layout,
later async states, interactions, screenshots or visual design acceptance.

The layout derives from Herdr app 0.1.12's main layout with agents/Spaces/tabs/pane
references replaced, a 320px sidebar and a content-sized Spaces block. Its Apache-2.0
license and provenance travel inside mods/panorama/. No artwork is copied; shipped mod/asset references stay intact.
