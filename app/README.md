# herdr-app flavour scaffold

B1 supplies installation and verification infrastructure. All four panorama mods
are intentionally empty native Views. The designed sidebar, spaces, graph and
Peek arrive in later slices. Installing does not select the panorama layout.
The TUI plugin is independent and is not changed by this installer.

From a reviewed checkout, with an existing, canonical absolute app-home path:

```sh
scripts/app-install install --app-home /absolute/path/to/herdr-app
scripts/app-install uninstall --app-home /absolute/path/to/herdr-app
```

Linux or macOS, Python 3.11+ and the native `herdr-app` executable are required. The installer
checks a private copy of every installed mod, using a temporary app home, unique HC_SESSION,
and scratch XDG config/state/runtime paths with inherited HERDR_/HC_ endpoints
removed. Checker-generated types/config stay in scratch. It never calls layout.switch, layout.clone, hc or the TUI installer.
After installation it prints the command to switch when the person chooses.
No programs, file reads or network operations run from the empty scaffold mods.

An optional explicit reviewed executable is copied privately:

```sh
scripts/app-install install --app-home /absolute/path/to/herdr-app \
  --binary /absolute/path/to/reviewed/panorama
```

The destination is `data/panorama/bin/panorama`, mode 0755. There is no PATH lookup
or global installation for panorama. Later mods use this exact app-home path;
B1 does not execute the binary. A missing artifact is valid for B1.

The installer refuses existing panorama mod names, layout/file destinations,
symlinks in app-home paths, hardlinked destination files and all binding
collisions, even identical actions. It never renames a collision silently.
`app/keys.json` is empty until a design specifies bindings. Nonempty merges are
covered with explicit test packages, including collision refusal and exact raw
original key-byte/mode restoration.

Operations lock the app-home directory itself, leaving no lock-file residue.
New files use atomic create-only rename (Linux renameat2 / macOS renameatx_np)
through stdlib ctypes, with no overwrite or link/unlink fallback.
A mode-0600 `data/panorama/install.json` records file hashes/modes, created
directories and key additions before payload writes. Failed native checks roll
back unchanged installed files, including writes whose directory fsync failed
after publication. Incomplete rollback retains its ownership journal for recovery.
After an interrupted install, run uninstall before trying to install again. Abrupt termination before the journal is created
can leave new temporary files/directories; no existing files change in that interval.
Filesystem power-loss guarantees are outside this installer.

Uninstall validates its bounded package inventory before deleting anything.
Key-removal authority comes from this checkout's package key definitions, not
from a self-consistent manifest delta; use the same reviewed package for uninstall.
Modified files, permission changes, added files and changed bindings survive;
concrete leftovers are printed. For later key edits, only unchanged bindings
added by this install are removed. Malformed edited keys are preserved whole.
A substituted symlink or unsafe manifest refuses uninstall without following it;
resolve that explicit conflict before retrying. Uninstall does not delete the
feed writer's runtime data, which is not owned by installation. A current layout
that references removed mods is the person's responsibility; uninstall never
switches layouts.

```sh
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

The layout derives from Herdr app 0.1.12's main layout with only agents and spaces
mod references replaced. Its Apache-2.0 license and provenance travel inside
`mods/panorama/`. No artwork is copied; shipped mod/asset references stay intact.
