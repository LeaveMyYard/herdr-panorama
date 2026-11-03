# App-feed runtime ownership v1

Root-approved A3/B2 interface. Runtime ownership is separate from the UI feed. No PID or process-group signal is an uninstall operation.

## Receipt and locks

New installs use the existing `data/panorama/install.json` receipt with exactly
`{v:2, install_id, files, dirs, keys}`. `install_id` is 32 lower-case hex digits,
new for each install; it is not reused across uninstall/reinstall. Existing
`files`, `dirs`, and `keys` keep the installer's checked inventory semantics.
`files` contains only static package payload plus the optional private binary,
never mutable runtime/control/ownership fingerprints. Explicit fingerprinted
`gpui-kit.d.ts` and `jsconfig.json` checker metadata may be included at mode 0644
only inside the owned panorama mods; the installer stages native-checker bytes,
verifies their equality and journals them before publication. Generated metadata
is not committed to this repository or adopted from personal files. `dirs` may also record
created `runtime` and generation directories. The finite additional empty host mod-data
reservation allowlist is `data/panorama-spaces`, `data/panorama-graph` and
`data/panorama-peek`. Reserve and journal these directories BEFORE loading any
owned module; never adopt a preexisting directory. Cleanup uses rmdir only when
the exact owned directory is empty. Added files remain personal: preserve/report,
never recursively remove them. `data/panorama` is the explicit writer inventory
above, not a generic empty mod-data reservation. No `shared.json` or `store.json`
cleanup is authorized. Host shared bookkeeping is outside this deletion authority;
observed residue remains an OUTSTANDING integration acceptance gap until a
supported safe cleanup API or an explicit operator decision resolves it.
The no-trace requirement is not waived. Any future owned-mod name
requires an explicit finite-inventory handoff before reservation.
Legacy v1 receives only its original static uninstall behavior; it never grants
writer/runtime authority. There is no in-place receipt upgrade or dev bypass.

`--ensure` requires v2, its exact binary entry
`files["data/panorama/bin/panorama"] = {sha256:<64 lower-case hex>,mode:493}`,
the actual regular binary's SHA-256 and mode 0755, and invocation of that exact
private artifact. A missing binary, hash/mode/path mismatch, unsafe path or
ambiguous receipt is an error. No PATH search for panorama and no global install.

All operations open no-follow directory/file descriptors under the explicit
absolute app home. Receipt and runtime files are regular, single-link, mode 0600.
Static mods/layout/licenses retain mode 0644; the installed binary is 0755.
Symlinks, unsafe types and unexpected replacements refuse.

| Lock | Inode | Holders and lifetime |
|---|---|---|
| Lifecycle | Existing app-home directory FD, flock EX | Installer install/uninstall and ensure validation/start; bounded acquisition for ensure. Existing installer lock has this identity. |
| Writer | Generation's `writer.lock` file FD, flock EX | One writer for its whole life; parent ensure reserves it before spawn and hands the same open-file description to the child on stdin. |

Writer exit never acquires the lifecycle lock. Uninstall keeps the lifecycle
lock while disabling and waiting, then acquires and holds the writer lock
through runtime cleanup and receipt removal. Ensure cannot restart between
stop and cleanup. Delayed ensure after receipt removal refuses to start.

## Exact paths

Let `G = data/panorama/runtime/<install_id>`; all paths below are relative to
app home. The installer reserves/creates G, control.json and the empty writer
lock before publishing a usable v2 install. Existing collisions refuse; a new
receipt cannot adopt an old generation directory or preexisting feed.

| Path | Creator | Format / removal authority |
|---|---|---|
| `data/panorama/install.json` | Installer | v2 receipt above; removed last under lifecycle lock. |
| `data/panorama/bin/panorama` | Installer | Exact fingerprinted private binary. |
| `G/control.json` | Installer / failed-start ensure | Control below; false initially, true on uninstall/failed start. |
| `G/writer.lock` | Installer | Empty regular 0600 file; never truncated/replaced while in use. |
| `G/ownership.json` | Writer | Ownership below; generation and binary fingerprint must match receipt. |
| `G/.ownership.tmp` | Writer | Exclusive-create staging; any leftover is ambiguous and preserved/refused. |
| `G/.control.tmp` | Installer / failed-start ensure | Exclusive-create staging for control replacement; leftover is ambiguous and preserved/refused. |
| `data/panorama/feed.json` | Writer | Atomic full feed v1; bytes must match a recorded ownership digest before replacement/removal. |
| `data/panorama/.feed-<install_id>.tmp` | Writer | Exclusive-create sibling staging; removable only if complete bytes match pending digest for this generation. |

The shared `data/panorama` parent contains only `install.json`, `bin`, `runtime`,
the optional `feed.json` and this generation's optional `.feed-<install_id>.tmp`.
A foreign-generation feed staging file is unknown runtime data: preserve it and
refuse ensure before spawn, even if its bytes are complete JSON. No adoption.

No glob cleanup. Unexpected runtime files, unknown generations, invalid/partial
staging, runtime provenance/digest mismatches and stop timeouts are preserved,
with a concrete error, disabled control and the receipt retained. After runtime
quiescence and ownership validation, edited/added static mods and an edited
private binary keep B1 behavior: preserve/report those personal static files,
remove unchanged owned files and the receipt, and succeed. This static policy never applies to runtime ambiguity.
Directories are removed only when owned and empty. A crash can leave an ambiguous
staging file; preserving it is mandatory.

## JSON control and ownership

All objects have exactly the listed keys, no duplicate keys; integer fields use
safe integral numeric semantics (booleans are not integers). Install ids and digest
spellings are strict. Receipt reads are capped at 1 MiB; control and ownership
reads, including their staging files, at 4 KiB each. Complete feed frames and
their pending staging files are capped at 32 MiB each. Writer PID is an integral
number in [2, 4294967295]. writer_start and a non-null writer_pidns each contain
1–256 UTF-8 bytes and no NUL, CR or LF; null namespace remains valid on macOS.

Control:

```json
{"v":1,"install_id":"0123456789abcdef0123456789abcdef","disabled":false}
```

The installer or an ensure operation whose child start failed after validation
changes disabled to true under the lifecycle lock, atomically via `.control.tmp`.
A validation failure never adopts or changes an ambiguous control.
Ensure and writer refuse disabled, absent, foreign-generation or malformed
control. A writer that idles out leaves false control, so a later app start can
ensure again in the same installation. Uninstall never re-enables control.

Ownership:

```json
{"v":1,"install_id":"0123456789abcdef0123456789abcdef","binary_sha256":"<64 lower-case hex>","writer_pid":4242,"writer_start":"fixture-start","writer_pidns":null,"feed_sha256":null,"pending_feed_sha256":null}
```

`writer_pid` is >1, writer_start is a nonempty bounded start token, and
writer_pidns is a nonempty namespace string or null. These are diagnostics and
busy-writer proof, never signal targets. Ensure reports an existing writer only
when its registration matches generation/binary and current PID/start/namespace
and exact private command address; otherwise busy ownership is ambiguous and
refused. Uninstall does not need that process to still exist: quiescence is the
exclusive writer lock, and artifact authority is generation plus digests.

The two nullable digest fields journal atomic feed publication:

1. Validate current feed against feed_sha256 (or require absence when null).
2. Atomically publish ownership with pending_feed_sha256 = new complete bytes'
   digest, retaining the old digest.
3. Exclusively create/fill/fsync the sibling feed temp; recheck control and the
   current feed fingerprint, then rename over feed.json and fsync its directory.
4. Atomically commit ownership: feed_sha256 = new digest, pending = null.

After a crash between steps, the current complete feed is owned if its digest
equals either recorded digest. A resumed writer can finish/reconcile that state
under its lifetime lock. An uninstaller accepts either recorded digest after
quiescence. No successful publication can authorize deleting unrelated bytes.
Uninstall never executes the current private binary and does not require its
current bytes to match startup's binary check. It disables and waits using the
valid receipt/control/lifetime lock only. Runtime ownership.binary_sha256 must
match the ORIGINAL digest in the receipt; when that provenance and feed digests
are proven, runtime cleanup proceeds even if the on-disk binary was edited.
The edited binary is then preserved/reported as personal static payload and the
receipt removed. A runtime provenance mismatch, unknown runtime addition or
stop timeout still refuses and retains the disabled receipt. Ensure/start
always rejects a modified actual binary; this uninstall policy never relaxes
startup checks.
The initial registration has both digest fields null and requires no feed.
All ownership replacements use exclusive `.ownership.tmp` creation; failure
leaves the last valid feed intact. No receipt or control means no new writes.

## Starting, stopping and timing

`panorama app-feed --help` is a non-starting, no-files capability probe, exit 0,
with `panorama app-feed v1` in stdout. Public startup is
`app-feed --ensure --app-home ABSOLUTE_HOME [--install-id GENERATION]`;
default home matches herdr-app:
HERDR_APP_HOME, otherwise HOME/.config/herdr-app. No alternate development mode.

Optional `--install-id` pins startup to exactly that 32-lower-case-hex receipt
generation. Malformed or duplicate generation arguments exit 2 before any I/O.
A supplied generation differing from the validated v2 receipt exits 1 before any
spawn. Omission is for manual invocation and targets the current receipt. The
child still receives the currently validated generation and reserved lifetime FD;
its existing receipt/control/FD proof remains mandatory.

Every frontend client pins the generation from its initially validated receipt
and supplies it on every non-help ensure/Inbox call. It re-reads the receipt and
private artifact fingerprint before each call. A changed or missing receipt
permanently stops that old client; retained feed data stays stale, never becomes
fresh evidence for another generation. Uninstall/reinstall therefore cannot turn
a delayed old-client request into a new writer or navigation request.

Ensure holds the lifecycle lock, validates the receipt/artifact/control/state,
reserves the lifetime lock, and spawns only the exact private executable. Child
stdin carries the reserved lock FD; child validates its inode and ownership.
The parent waits at most 2 s for a generation-specific ready acknowledgement;
the child registers ownership before acknowledgement, before slow collection.
Failure after a validated spawn disables this generation and returns an error;
no PID is killed. A
ready acknowledgement does not promise that the first feed is already collected.

Writer control checks run independently of collection at most 100 ms apart.
Disabled/removed receipt or control requests cooperative shutdown; collection
helpers are cancelled/reaped through their owned handles. The lifetime lock is
released only after owned helpers stop. Installer waits at most 2 s, polling
the writer lock every 25 ms; timeout preserves/refuses with receipt and disabled
control retained. It never force-kills the watcher or bypasses a busy lock.

App liveness comes from `HERDR_APP_HOME=<exact home> herdr-app status`, not PID
names or env presence. Installed herdr-app CLI 0.1.12 observation: live status exits 0 with
a JSON object; absent window exits 2 with `herdr-app: no Herdr window at ...`.
Other errors/timeouts are unknown, reported as data, not proof of absence.
Writer idles out after 30 s of confirmed absent-window observations; unknown
observations reset that absence interval. Probe every 2 s, bounded to 500 ms.
An idle exit retains the last feed and registration as stale data, not live app
membership. Normal collection polls changed source signatures and at least
every 5 s as a fallback; runtime control never enters UI membership fields.

## Shared evidence

`tests/fixtures/app-feed-runtime/lifecycle.json` supplies the same generation,
paths, control and ownership examples for Rust and installer/Node tests. Tests
must exercise a used temporary home, concurrent ensure singleton, cooperative
stop/uninstall, delayed ensure after receipt removal, restart prevention under
the lifecycle lock, preserved personal/unknown files and interrupted ownership.
No live installation, personal configuration or global executable is modified.
