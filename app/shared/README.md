# Native shared source

Edit these canonical modules, then run `scripts/app-build`. Each mod declares
its required modules in `bundle.json`; generation checks dependency closure and
copies them into that mod's own directory. `scripts/app-build --check` detects
missing/stale copies in CI. Generated runtime modules are committed and listed
explicitly in the installer's inventory.

herdr-app confines module imports to one mod directory. A real native probe with
two owned temporary panorama modules rejected a sibling import, so runtime mods
must be self-contained. No mod reads another mod's code at runtime.

Current exports for later app slices:

- `contract.js`: feed v2 (`docs/protocol/app-feed-v2.md`) `parseFeed`, `readResult`,
  `rootKey`, `agentKey`, and the capability probe `FEED_PROBE` / `probeMatches(stdout)`
  (first stdout line exactly `panorama app-feed v2`).
  Render identities are tagged opaque namespaces, not terminal addresses.
- `model.js`: `joinRoots`, `groups`, `rootSignature`, `harnessOf(agent, root)`, `iconProgram`,
  `rootTitle`,
  `terminalChildren`, `nativeMembership`, `spaceTotals`.
- `tree.js`: `needs`, `finished`, `urgency`, `forest`, `aggregate`, `agentTitle`,
  `subtree`, `live`, `directTiles(root, natives)` (line-2 tiles per direct subagent),
  `annotate`, `isOpen`, `rows` (the V11 docked list: layer-1/layer-2 agent rows and the
  root's `N finished · titles · +N older` fold).
- `marks.js`: native `icon`, `mark`, `rootMarks`, `lines`,
  `borderMotion`, `age`. It imports native gpui APIs and is not a Node module.
- `peek-caller.js`: `PeekCaller`; caller-local current-G scope, token/owned-overlay lifecycle and Transcript replacement. No persisted keys.
- `routing.js`: `rootTarget`, `detailTarget`; only model-verified addresses can
  produce a root command. Every detailed transcript agent targets Peek.
- `state.js`: the V10 state vocabulary (`docs/design/herdr-app/state-refresh-v10`).
  Native gpui, not a Node module. Exports `STATES` (code → title, label, text,
  hue, fill, ink, glyph, mute), `SURFACE`, `NEEDS`, `GLYPH_DIR`, `stateCode`,
  `stateOf`, `mix`, `cardColors(status, selected)` → `{edge, fill, border}`,
  `glyph(status, size=11)`, `badge(status, label?, {questions, height})`,
  `tile(status, size=18|16|14, count?, {still})` and
  `glyphRun(statuses, limit=6, finished=0, size=11)`. Statuses may be feed codes
  (`run`, `blk`, …) or native words (`working`, `blocked`); unknown codes draw as
  Unknown. Colours are Panorama-scoped (TUI `palette.rs` ST/PB), not `theme()`.
  Running glyphs spin natively (`.spin(900)`); needs-you tiles breathe 70–100%
  unless `still`. Badges hug their label inside a row; put them in an `h_flex`.

Shared glyphs live in `glyphs/*.svg` (tinted masks, `currentColor`). A mod whose
bundle lists `state.js` gets byte-identical `state-<name>.svg` copies, and its
`state.js` copy has `__PANORAMA_MOD__` replaced by the mod name, so
`svg("mods/<mod>/state-<name>.svg")` resolves inside that mod. The installer
inventory lists them as `STATE_FILES`; receipts from before V10 stay valid.

`joinRoots` returns top-level stock entities plus unmatched feed roots. Every
stock `agents()` row remains top-level. Each entity has key, root (feed data),
stock (stock AgentInfo or null), terminal (verified live address or null),
name/title_source/spaceName, space/tab, status/program, pinned/focused, seq/changed, aggregate,
`nativeChildren` (array) and `isNative` (boolean). Unmatched roots have null app
membership/target and no invented changed-age. Entity `name` follows the app title rule (`rootTitle`, plan §2.2 and D3/D6): an
operator-named terminal, then an operator-named tab holding exactly one agent root,
then the feed `title`, then the telling live pane title, then the program name and
address; the space name is secondary (`spaceName`). Native child entities are in the
owning top-level entity's flat `nativeChildren` array, with depth/nativeParent,
verified inherited/direct membership and optional exact-address feed enrichment.
Display recursion stops at stock-row boundaries. Stock roots focus; native
pane-less children use terminal.peek. Detailed agent IDs never become addresses.

Aggregate fields are kinds (ordered fail/blk/wait), needsCount, questions,
working, sweep, priority and urgency. A root with non-empty `bg` and no needs or work gets status
`bg` (Waiting, priority 1.5, after the projection below). A top-level aggregate projects urgency from ALL verified live native
descendants, including separately listed roots; display ownership still stops at
listed-root boundaries. `spaceTotals(roots, spaceId)` recomputes OWN aggregates
from each canonical top/native entity with exact proven space membership, dedupes
qualified terminal addresses, and unions scoped root/session/question IDs. It
returns `{kinds, needsCount, questions, working, done}` and never adds projected
parent aggregates. Own done stays Done unless needs; own idle plus a working
descendant is Working; child done alone never promotes Done. Unmatched roots have no space contribution. Agent parent links stay
inside their owning feed root and are independent of native terminal ancestry.

## Shared-v3 feed consumer and private calls

`feed-client.js` exports `startSidebar(view,cx)`, `startConsumer(view,cx)`,
`consumerFeed(view)` and `closeConsumer(view,cx)`. Start ONCE in View.init; it
returns synchronously and also sets `view.feedClient`. Store the returned
`client.callPrivate` closure if needed; it accepts ONE argument, an argv array,
and retains no caller context. Invoke it inside the caller's own cx.spawn.
Do not duplicate receipt hashing, startup, or private executable dispatch.
Use `consumerFeed(this)` in render, and `closeConsumer(this,cx)` in View.close.

The full returned packet is `{feed, stale, aged, errors, generated_at, writerError,
install_id}`. Feed is the retained last valid v2 snapshot or null. Preserve stale
and errors; do not unwrap `.feed` and discard its provenance. `install_id` is the
pinned generation or null during initialization. Pinning alone does not make a
view fresh: consumerFeed stays stale until receipt, control and artifact
initialization proof completes. A missing or changed receipt
permanently stops the old client, retaining stale data without retrying another
installation. Invalid artifacts never execute. Early private calls await the
initial identity and fail closed through their returned promise.

Owner publication and every consumer projection conservatively mark collection
time stale when `generated_at` is missing/nonfinite, in the future, or older
than 15 seconds (three normal 5-second collection polls). Data and collection
time remain retained. This is a view freshness window, not proof of writer
death, a heartbeat guarantee, or membership/command authority; a renewed frontend
lease cannot make an old collection fresh.

`aged` is that collection-age test alone (`aged` in `contract.js`), and it implies
`stale`. A view that renders `joinRoots` must pass `{ aged: packet.aged }`: terminal
numbering is per core instance, so an aged collection joins no live row. A caller
that refuses `stale` first, as Peek does, need not pass it.

After its startup ensure has run, successfully or not, a current-generation owner
may idempotently re-ensure an aged feed, and also, whatever the feed's age, until one
ensure has succeeded and while a failed retry's error stands. It does so through
the existing receipt/hash/generation-verified
private bridge. The existing 5-second heartbeat permits at most one pending
recovery and one attempt per interval, with owner/epoch checks after awaits.
Success only acknowledges startup: old collection bytes remain stale until a
current collection timestamp is read. Followers independently age their cache;
they add no competing reader/recovery timer. Disabled, missing or changed
installation authority still permanently stops that client without retry.

The client exposes `callPrivate(args)`, read-only `install_id` and `stopped`, and
`close(cx)` (the latter is normally called via closeConsumer). The only permitted
argv are `[app-feed,--help]`, `[app-feed,--ensure]`, `[app-inbox,--help]`,
`[app-inbox,--available]`, `[app-inbox,--open]`, or
`[app-inbox,--open,--question,ID]`; each spelling represents literal argv strings.
Question ID is canonical ASCII positive decimal <=9007199254740991. A trailing
`--app-home,appHome()` is accepted for consumer compatibility but cannot select
another home. Every non-help call injects that exact home and `--install-id,G`.
Each call re-reads the receipt and rehashes the fixed artifact, rejects changed
G, and invokes only native herdr.exec. The backend fence protects the remaining
receipt-generation race. Nonzero result codes remain caller-visible; opening
Inbox/navigation and duplicate-dispatch UI remain the consumer's responsibility.

Shared `panorama.feed.owner` is advisory `{v:1,token,role,seen_at}` only, heartbeat
5s/TTL15s. It does not authorize a writer, installation identity, or fresh data.
Sidebar gets initial priority. A consumer falls back to file/startup ONLY if the
current layout contains NO panorama reference; hidden references count present.
All ordinary claims validate current receipt, enabled current-generation
control and artifact before creating any owner object. An initial no-sidebar
consumer may claim once over a persisted lease; latest claim wins and losing in-flight work checks its token after every await and
before publishing. Later heartbeats only reclaim invalid/expired leases.
Nonfinite/future timestamps are invalid. Close cancels the timer, logically
cancels pending file work, never re-arms its watch, and releases only its own
lease through the token/generation guarded final-close rule below. The host has no watch-promise cancellation API. A persisted packet without
a valid local pin/current advisory owner is projected stale.

`binary.js` exports installReceipt/verifiedBinary for this client; these are
internal validation helpers, not executable authority for consumers. Its native
crypto/createHash and fs/promises/readFile adapters were explicitly approved.
There is no stat/no-follow API and no atomic identity guarantee. The 256MiB
acceptance bound applies AFTER reading, not before allocation. The executable
revalidates the complete filesystem proof under its lifecycle lock. The tested
host additionally refuses fs.readFile above 64MiB (a 142MiB debug artifact failed
closed); the actual merged ~10MiB release-profile artifact passed native
consumption/active-uninstall/reinstall fencing. This observed host limit is not a
new general pre-read API contract. Use an appropriately sized reviewed artifact.

A current-generation disabled control permanently stops the client. A newly
opened current mod can recover its crashed matching-G cached packet by a
synchronous claim-and-null, without invoking a helper; foreign/missing packets
cannot be claimed for recovery. Guards revalidate receipt/control after every
await before using file/private-command results to claim or publish. Delayed
results after stop/ack never restart the reader or republish its cache.

Changed-age state is view-local and creates no shared age-map key. Each live
view caches only its matching-generation packet. Explicit View.close or missing/changed/disabled authority stops release ONLY
this client's current token. Feed nulling additionally requires the packet
install_id to match its pinned G. Early close may release its own token even
without a matching packet; it cannot clear foreign/new-generation feed.
Ordinary lease loss/handoff does not clear shared values. An old client cannot
clear a new owner's token or another generation's packet. Followers retain their
matching-G cache STALE after an owner stops; live unavailable views retain stale
provenance locally. No full cached transcript remains in shared state after a
normal ready owner's final close.

The host persists shared keys: native probes show undefined and null both leave
a key with null. The actual used-home close/uninstall probe leaves only
`panorama.feed:null` and `panorama.feed.owner:null`, with no changed-age key.
The operator explicitly approved ONLY these two inert null slots after
uninstall. Cached feed, changed-age map and other Panorama payload residue are
not approved. The immutable private shared-v2-final handoff predates this narrow
approval; this canonical documentation supersedes its pending-decision wording. Do not edit global shared.json or arbitrary local stores.

`validLease` and `privateArgs` exports support isolated tests; application
consumers should use the four lifecycle functions and the bound call closure.
