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
created `runtime` and generation directories. The finite host mod-data
reservation allowlist is `data/panorama-spaces`, `data/panorama-graph`,
`data/panorama-peek`, `data/panorama-tabs` and `data/panorama-pane`. New directories
are reserved/journaled BEFORE module load, and may be removed only when empty.
Existing graph/Peek/pane directories are not adopted.

The operator explicitly approved retaining exactly
`data/panorama-spaces/store.json` and `data/panorama-tabs/store.json` with their
necessary parent directories. These hold stock panel selection, machine folding
and machine-reconnection recovery state; ordinary restarts preserve them. Graph
changed-age state is View-local and has no persistent store. The exact two
preference parents may preexist on reinstall: safely open without following
links and read only a present known store as a regular single-link mode-0600
file, capped at 1 MiB, strict UTF-8 JSON string-map with consumed known preference
shapes checked. Unknown preference keys/files remain untouched. Unsafe known
stores refuse before mutation. Existing parents are NEVER added to a new
receipt's created `dirs`; reuse grants no deletion ownership. The installer
never rewrites/chmods/removes these stores. Newly created parents are tracked
and rmdir is attempted only if empty; retained files/parents are reported.

Added personal files remain preserved/reported, never recursively removed.
`data/panorama` is the explicit writer inventory above, not a generic mod-data
reservation. No direct global `shared.json` edits are authorized. The only
approved shared residue is `panorama.feed:null` and `panorama.feed.owner:null`.
Together with the exact two retained preference stores/necessary parents and the
separately approved host-owned `layouts/.state/panorama.json` snapshot/necessary
parent, these are the operator's complete no-trace exceptions. Preserve that exact
layout-state snapshot without granting installer deletion, adoption, rewrite or
chmod authority; it adds no receipt or runtime-inventory entry. Cached feed/transcript
payload, graph age maps and other owned residue are not approved.
The six-mod installer refuses older four-mod B2 v2 receipts before mutation;
uninstall with the reviewed package that created that receipt before installing
the six-mod version. There is no in-place receipt/profile adoption. Legacy v1
receives only its frozen original four-mod/eleven-required-path static uninstall
behavior, optional private binary/modes and original key authority; it never grants
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
| Lifecycle | Existing app-home directory FD, flock EX | Installer install/uninstall and ensure validation/start; ensure waits at most 10 s for it (one concurrent ensure's acknowledgement window plus its stop wait). Existing installer lock has this identity. |
| Writer | Generation's `writer.lock` file FD, flock EX | One writer for its whole life; parent ensure reserves it before spawn and hands the same open-file description to the child on stdin. |

Writer exit never acquires the lifecycle lock. Uninstall keeps the lifecycle
lock while disabling and waiting, then acquires and holds the writer lock
through runtime cleanup and receipt removal. Ensure cannot restart between
stop and cleanup. Delayed ensure after receipt removal refuses to start.

After disabling and proving writer quiescence, uninstall also waits for frontend
acknowledgement before cleanup, deleting static payload or removing the receipt
last. Keep disabled control and the receipt present throughout this wait.
Poll at 25 ms for at most 2 s under the lifecycle lock. A frontend observing its
current pinned generation disabled permanently stops that client and clears its
owned shared state only through the supported API. It may release a lease whose
token it owns without a matching packet; clearing the feed requires BOTH its own
lease token and a packet matching its pinned generation.

The installer only READS `<appHome>/shared.json`: no-follow, single-link regular
file, at most 8 MiB, strict duplicate-free JSON object. Inspect ONLY
`panorama.feed` and `panorama.feed.owner`. A missing file or both keys absent/null
is acknowledgement. Foreign/ambiguous data, malformed/oversized/unsafe files,
read refusal or timeout preserve disabled control, receipt and static payload;
never edit the shared file to manufacture acknowledgement. A crashed app with
nonnull data refuses. The user may manually reopen the current mod to clear its
disabled-generation state and retry; the installer never launches the app or
switches layout. The exact two-null-key operator exception above is unchanged.

A new view validates the current receipt and control BEFORE ordinary lease claim
or publication. Generation/control guards apply across every await: revalidate
before using asynchronous results to claim or publish. Disabled-generation
recovery may claim and null ONLY a packet matching its pinned generation; it
cannot claim foreign or missing packets. The own-token lease-release rule above
still applies independently of packet matching.

The first shared acknowledgement is checked under lifecycle and writer locks.
Retain validated bytes/open FDs, original mode and digest ONLY for unchanged owned
static entries actually removed by this invocation. After static removal, perform
the same FINAL read-only shared check BEFORE key undo, runtime control/lock cleanup
or receipt-last success. Late nonnull/ambiguous data refuses: restore ONLY those
recorded entries by exclusive no-replace creation with their original mode/digest.
Never overwrite a reappeared personal path. Report collisions or restoration
failures as partial recovery gaps; retain the receipt, disabled control and
lifetime-lock file, with keys untouched. Retry is supported only if the usable
owned payload was restored; do not claim usable recovery after a partial failure.
On success, the final check precedes key undo, runtime cleanup and receipt removal
last. No new runtime path or shared-file edit is authorized. These are bounded
validation/recovery rules, not universal crash atomicity across the host store
and filesystem. Frontend/installer race and restoration tests remain separate
integration proof; the Rust writer fixture alone does not establish acceptance.

## Exact paths

Let `G = data/panorama/runtime/<install_id>`; all paths below are relative to
app home. The installer reserves/creates G, control.json and the empty writer
lock before publishing a usable v2 install. Existing collisions refuse; a new
receipt cannot adopt an old generation directory or preexisting feed.

| Path | Creator | Format / removal authority |
|---|---|---|
| `data/panorama/install.json` | Installer | v2 receipt above; removed last under lifecycle lock. |
| `data/panorama/bin/panorama` | Installer | Exact fingerprinted private binary. |
| `G/control.json` | Installer / failed-start ensure | Control below; false initially, true on uninstall or a permanent start failure (see Starting). |
| `G/writer.lock` | Installer | Empty regular 0600 file; never truncated/replaced while in use. |
| `G/ownership.json` | Writer | Ownership below; generation and binary fingerprint must match receipt. |
| `G/start.json` | Ensure | Optional transient-start ledger below; created/replaced only while ensure holds the reserved writer lock before spawn, removed by ensure after a proven start or a permanent failure and by uninstall cleanup. |
| `G/.start.tmp` | Ensure | Exclusive-create staging for the ledger; leftover is ambiguous and preserved/refused. No tool removes it; an operator may ([recovery](../../app/README.md#uninstall-and-troubleshoot)). |
| `G/.ownership.tmp` | Writer | Exclusive-create staging; any leftover is ambiguous and preserved/refused. |
| `G/.control.tmp` | Installer / failed-start ensure | Exclusive-create staging for control replacement; leftover is ambiguous and preserved/refused. |
| `data/panorama/feed.json` | Writer | Atomic full feed ([v2](app-feed-v2.md)); bytes must match a recorded ownership digest before replacement/removal. |
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

The installer, or an ensure operation whose start failed permanently (see
Starting), changes disabled to true under the lifecycle lock, atomically via
`.control.tmp`. A transient start failure never changes control.
A validation failure never adopts or changes an ambiguous control.
Ensure and writer refuse disabled, absent, foreign-generation or malformed
control. A writer that idles out leaves false control, so a later app start can
ensure again in the same installation. Uninstall never re-enables control.

Transient-start ledger (optional; absent means no recorded failures):

```json
{"v":1,"install_id":"0123456789abcdef0123456789abcdef","failures":2,"retry_at_ms":1793717546321}
```

`failures` is the count of consecutive start attempts not yet proven started
(see Starting), a JSON integer in [1, 7]; `retry_at_ms` is the Unix-epoch millisecond time before which ensure
defers a new spawn. Control keeps exactly its three keys: frontends read control
strictly, so the ledger is a separate file rather than control fields. A
malformed (including a non-integer number such as `2.0`), foreign or out-of-range ledger, or a leftover `.start.tmp`, is
ambiguous: ensure refuses before spawn and uninstall preserves/refuses, as for
other runtime staging. This file was added after the first v1 text; it is an
additive runtime path, and control, ownership, receipt and feed schemas are
unchanged.

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
with `panorama app-feed v2` in stdout. Public startup is
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
The parent waits at most 5 s for a generation-specific ready acknowledgement;
the child registers ownership before acknowledgement, before slow collection.
A ready acknowledgement does not promise that the first feed is already
collected. No PID is ever signalled.

Each spawn is counted before it happens: with the lifetime lock reserved (so no
writer runs and no writer can observe `.start.tmp`), ensure records the next
ledger failure n, unless n would be the 8th, which is not recorded. Only a
proven start removes that count, so an attempt that is never proven, including
one left starting that later exits without registering, stays inside the bound.
A ledger write that fails refuses before spawn without changing control.
Outcomes after a validated spawn:

| Outcome | Classification | Effect |
|---|---|---|
| `READY <install_id>` | Started | Remove the ledger; exit 0. A later ensure that proves this writer busy also removes a ledger. |
| No acknowledgement within 5 s, but ownership names this exact child and its PID/start/namespace/command are live | Started (late acknowledgement) | Same busy-writer proof a concurrent ensure accepts; remove the ledger; exit 0. The writer ignores a closed acknowledgement pipe. |
| No acknowledgement within 5 s and no such registration | Still starting, counted | Exit 1; control unchanged; the attempt stays counted; the child is left to register or give up. A later ensure proves it busy (removing the count) or finds its lock free. |
| Spawn error | Transient | The attempt stays counted; exit 1. |
| Child exits before acknowledging, and a fresh lifetime-lock description is acquired within 2 s | Transient | The attempt stays counted; exit 1. |
| Child exits before acknowledging, lock still held after 2 s | Permanent | Disable; remove the ledger; exit 1; receipt retained. |
| Any bytes that stop being a prefix of `READY <install_id>\n` (other generation, garbage) | Provenance failure, permanent | Disable; remove the ledger; wait 2 s for cooperative stop; exit 1. |

Recording failure n sets `retry_at_ms` to now + min(5 s × 2^(n−1), 160 s).
While that time is in the future (and at most 160 s away, so a clock moved
backwards cannot defer indefinitely), ensure exits 1 before spawn without
changing anything. The 8th consecutive attempt that fails transiently or is
left starting unregistered disables the generation instead (a starting child
then stops cooperatively); only a reinstall recovers from that, as before. A
ledger removal that fails is reported as a warning on stderr and leaves the
changed ledger for review. Validation and provenance failures before
spawn (receipt, generation, binary hash/mode/path, control, runtime names,
journal/ownership proof, ledger) never spawn and never change control; they
refuse on every ensure while the condition holds. A child that refuses its own
validation after spawn exits before acknowledging and is therefore counted as
transient; ensure repeats that validation before each later spawn. Debug builds
of the binary accept a test-only `PANORAMA_TEST_START=<register|ready>:<fault>`,
where the fault is `<ms>` (delay), `exit`, `hold-exit` (exit while a helper
keeps the lifetime lock), or at `ready` only `foreign` / `garbage` (a wrong
acknowledgement); release builds have no such knob.

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

The frontend conservatively projects a retained collection as stale when its
`generated_at` is missing/nonfinite, future, or older than 15 s (three normal
polls), independently of its advisory frontend lease. It retains data and age.
This view policy is not writer-death proof, a heartbeat guarantee, or membership
authority. After its startup ensure has run (successful or transiently failed), the current owner may use its
existing verified current-G bridge to idempotently re-ensure an aged feed, at
most once per existing 5 s heartbeat with one pending attempt and owner/epoch
guards after awaits. Ensure success alone never refreshes old collection time;
consumers independently age cached packets. Missing/changed/disabled authority
remains a permanent client stop; a non-disabled ensure error (including a
deferred start) is transient and may be retried at that cadence.

## Shared evidence

`tests/fixtures/app-feed-runtime/lifecycle.json` supplies the same generation,
paths, control and ownership examples for Rust and installer/Node tests. Tests
must exercise a used temporary home, concurrent ensure singleton, cooperative
stop/uninstall, delayed ensure after receipt removal, restart prevention under
the lifecycle lock, preserved personal/unknown files and interrupted ownership.
No live installation, personal configuration or global executable is modified.
