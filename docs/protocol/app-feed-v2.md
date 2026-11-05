# herdr-app feed v2

This is the Rust collector → herdr-app mod contract for the approved
[plan](../plans/herdr-app-mod.md), implementing the data needed by
[SPEC](../design/herdr-app/SPEC.md). Version 2 replaces
[feed v1](app-feed-v1.md) with one uniform agent tree per root
([`uniform-agent-tree.md`](../plans/uniform-agent-tree.md) §2 and §3, slice U7):
there are no threads or phases, and every agent below a root is the same kind of
node, linked by `parent`. It is a full snapshot, never a delta. It extends
`snapshot --json` (`src/snapshot/json.rs`) with `v`, root `host` and `terminal`,
and agent `harness`. It does not change that command or the TUI panel protocol.
The runtime that writes it, its receipt, generations and ownership are
[app-feed runtime v1](app-feed-runtime-v1.md), unchanged by this version.

## Delivery and validation

The writer publishes UTF-8 JSON at `<appHome>/data/panorama/feed.json`
(normally `~/.config/herdr-app/data/panorama/feed.json`). It writes a sibling
temporary file and renames it over the destination, so readers see complete
snapshots. `panorama app-feed --ensure` starts one detached watcher; repeated
calls are idempotent. The watcher recollects on change, with a slow polling
fallback, and exits after the app is no longer running.

Readers require a JSON object and numeric `v` equal to 2. Equivalent JSON
spellings such as `2.0` and `2e0` are accepted. Missing version, `true`, another
version (including 1), missing required fields, and wrong types reject the snapshot.
All fields in the tables below are required, including nullable fields and empty
arrays. `?T` means T or null, `[T]` means an array, and object names refer to other
tables. `number` is finite POSIX seconds (fractional seconds allowed); `integer`
means a finite integral numeric value within ±9007199254740991 (JavaScript
`Number.isSafeInteger` semantics); decimal/exponent spellings are accepted. Strings are plain text, not markup or commands.
Unknown fields are ignored for additive v2 compatibility; changing a type or
meaning needs another version. Unknown vocabulary strings remain valid and
visible using the fallback rules below.

On a missing, invalid, or unsupported feed, keep the last valid feed as stale and
continue displaying the stock agents. With no last feed, display stock agents
alone. Never turn a read/parse error into an empty successful swarm. `errors[]`
contains collector failures as sanitized plain strings; valid degraded feeds
remain usable, even when one source fails. `herdr_ok` describes the selected
host source (despite its legacy name); `inbox_ok` describes inbox availability.
`generated_at` is collection time, not a status-change timestamp or heartbeat
guarantee. A stopped writer leaves a stale file; readers must retain its age.

The capability probe `panorama app-feed --help` prints `panorama app-feed v2`
as its first stdout line. A v1 client refuses a v2 writer and a v2 client
refuses a v1 writer; a v1 reader of a v2 file (or the reverse) rejects it at `v`
and keeps its last valid feed as stale. The two versions are never installed
together: the receipt pins the mods and the private binary as one generation.

## Feed

| Field | Type | Meaning |
|---|---|---|
| v | integer | Exactly 2. |
| generated_at | number | Collection time. |
| herdr_ok | bool | Selected host source was read successfully. |
| inbox_ok | bool | Open inbox questions were read successfully. |
| errors | [string] | Partial collection/source errors; empty on success. |
| counts | Counts | Existing snapshot totals, not app section/space totals. |
| roots | [Root] | Visible roots; may have no transcript or agents. |

## Root

| Field | Type | Meaning |
|---|---|---|
| pane_id | string | Legacy source identity, nonempty; not necessarily an app pane. |
| host | string | `herdr-core`, `herdr-tui`, or `unknown`. |
| terminal | ?string | Exact core address, e.g. `t2` or `vps/t7`; null when unavailable. |
| harness | string | Harness identifier; unknown harnesses are retained. |
| tab_id | ?string | Legacy TUI tab id; core uses null. |
| tab_label | string | Legacy source label; display `title`, not this. |
| title | string | Cleaned display name, at most 80 characters; empty when no rule found one. |
| title_source | string | Rule that gave `title`: pane/tab/session/prompt/id, or unknown string. |
| session_id | ?string | Verified harness session identity; never a join heuristic. |
| herdr_status | string | Root runtime status, vocabulary below. |
| workspace_id | ?string | Legacy TUI workspace id; core uses null. |
| focused | bool | Legacy focus observation; core uses false. App owns focus. |
| cwd | ?string | Source folder metadata; never identity or app membership. |
| transcript | ?string | Transcript reference/path, or null when unavailable. |
| pending | [string] | Root blocking prompts, including permission prompts. |
| inbox | [integer] | Open question ids for this root/session. |
| inbox_texts | [string] | Question text parallel to inbox, same order and length. |
| agents | [Agent] | Flat array of every agent below this root, parents before children. |
| hidden_finished | integer | Nonnegative count of finished agents the done window omitted. |
| tab_no | ?integer | Legacy 1-based TUI tab position; core uses null. |
| machine | ?string | Legacy source machine label, not app space membership. |
| machine_id | ?string | Legacy source machine identity. |
| bg | [Background] | Root background work still in flight. |
| last_activity | ?number | Newest transcript timestamp, or null. |

## Agent

| Field | Type | Meaning |
|---|---|---|
| id | string | Nonempty agent identity, unique within its root. |
| harness | string | Explicit harness or verified parent/root harness; else unknown. |
| title | string | Cleaned display name, at most 80 characters; empty when no rule found one. |
| title_source | string | Rule that gave `title`: spawner/prompt/type/id, or unknown string. |
| description | string | Task description as the spawner wrote it. |
| role | string | Secondary display role. |
| agent_type | ?string | Harness-specific agent type. |
| model | ?string | Model display name, not inferred from harness. |
| status | string | Own status code, vocabulary below. |
| tool_use_id | ?string | Harness tool call identity when available. |
| started | ?number | Start time. |
| ended | ?number | End time. |
| last_at | ?number | Last observed agent timestamp. |
| tool_calls | integer | Nonnegative observed tool count. |
| activity | string | Current activity; display as supplied. |
| last_message | string | Last message for Peek. |
| questions | [string] | Open question text parallel to question_ids. |
| pending | [string] | Blocking prompts; these are not additional inbox questions. |
| question_ids | [integer] | Open question ids owned by this agent. |
| error | ?string | Failure detail for Peek; null when unavailable. |
| parent | ?string | Parent agent id in the same root; null means a direct child of the root. |
| depth | integer | Producer depth hint; reconstruct depth from parent links. |
| resumes | integer | Nonnegative resume count. |
| inferred | bool | Ending inferred rather than directly observed. |
| transcript | ?string | Transcript reference/path for read-only viewing. |
| seq | integer | Stable collector index, usable as deterministic tie-breaker. |
| bg | [Background] | Agent's background work still in flight. |

## Background

| Field | Type | Meaning |
|---|---|---|
| id | string | Harness background-task id. |
| kind | string | Harness background-task kind. |
| description | string | Task description. |
| started | ?number | Start time. |

## Counts

These retain `Swarm::counts()` semantics; consumers must not use them for app
section counts or space needs-you chips.

| Field | Type | Meaning |
|---|---|---|
| roots | integer | Number of roots. |
| agents | integer | Number of flat agent entries, never recursively counted twice. |
| running | integer | Agents with run or rev status. |
| waiting | integer | Roots with nonempty bg plus agents with bg status. |
| questions | integer | Sum of root inbox lengths, not agent question lengths. |
| blocked | integer | Roots whose herdr_status is blocked. |
| failed | integer | Agents with fail status. |

## Titles

A title is what every view calls a node; `role`, `agent_type`, the short id and
`model` are secondary text. The writer cleans every title (control and bidi
characters dropped, whitespace collapsed, leading decorations stripped, cut to 80
characters) and records the winning rule in `title_source`, which drives no
logic. An empty `title` means no rule found one (a harness without title rules);
a reader then falls back to `tab_label` for a root and to `role`, then `id`, for
an agent.

A core writer has no tab or pane data, so a `herdr-core` root's `title` comes
from its session title, first prompt or telling terminal title, with
`title_source` `session`, `prompt`, `pane` or `id`. The mod applies the
operator's names from live app data first (plan §2.2, D3): a pane name the
operator set, then the tab name when the tab is operator-named and holds exactly
one agent root, then the feed `title`, then the telling live pane title, then the
program name and address.

## Identity, tree and app joins

Use `(host, terminal)` for a core root with a terminal, otherwise `(host,
pane_id)`. Every root key must be unique; duplicate enrichment roots reject the feed.
Both are opaque, exact strings. Agent keys append agent id to the root key.
Agent ids must not repeat within a root and may repeat across roots. Question
keys append question id to the root key and its session_id; the same numeric
question on another machine/session is not the same question.

For herdr-core, match `terminal` exactly against `agents().address` and
`terminals().address`; locate app panes by `panes().terminal`, then use live
app space/tab data. Never join using cwd, title, pane_id, session_id prefixes,
agent titles, or guessed parentage. A TUI or unknown-host root cannot acquire
app membership from a coincidentally matching terminal string. Remote addresses
stay qualified: `vps/t7` does not match local `t7`.

Terminal numbering is per core instance, so a collection older than the 15 s
limit in [app-feed-runtime-v1.md](app-feed-runtime-v1.md) proves nothing about the
terminal now at an address. While the packet is aged the mods apply no `terminal`
join: stock rows and native children show live app data alone and count nothing
from the feed, and a root whose address has a live row is not listed a second
time. The retained feed stays cached with its age and returns when a current
collection arrives. Staleness that is not age (a lease handoff, a failed read of a
collection still inside the limit) keeps the join until the limit passes.

Legacy membership fields remain only to preserve the snapshot shape. Core
writers use the placeholders documented above and add no space/tab membership
fields. App focus, pinning, changed age, section ordering and icons come from
the matched stock agent; app space/tab membership always comes from live app
data. Every stock `agents()` row must remain present, including unsupported
harnesses and roots with no panorama transcript. Feed enrichment must never
filter those rows out. Unmatched feed roots remain in the sidebar under their opaque feed identity,
using supplied title, pane_id, harness and status as data. Never invent an
app space/tab/name/section/membership/focus for them, or portray stale/degraded
feed provenance as fresh live app evidence. They are excluded from space/graph
aggregates; a space graph contains only roots with a verified live membership join.

Native pane-less terminals may inherit membership only through a verified LIVE
terminals().parent chain to an exactly joined live stock ancestor. Direct child
pane/stock membership wins. Each agents() entry stays a top-level root,
including descendants with their own pane. Native sidebar recursion includes
only children absent from agents(), stops at listed-root boundaries, and gives
each root its own subtree. Preserve stock descendant urgency and count each
canonical native terminal identity once. Transcript agents remain separate;
never infer their address or membership from ids, cwd, titles or session names.

The root is the depth-0 node. `parent` links define an arbitrary-depth tree
over the root's whole `agents` array; `parent: null` is a direct child of the
root. Never truncate at a fixed depth. Render dangling and self-parent entries as
root children. As in `TreeKids::of`, entries unreachable from the root because of
cycles become root children, preserving every agent exactly once. Do not use
`depth` to guess missing parents. Retain producer order among siblings, with
`seq` as a stable tie-breaker where a view sorts.

Agent entries do not carry terminal addresses in v2. Detailed agents are
therefore pane-less; do not interpret their ids as terminal addresses. Every
detailed agent opens Peek; its Open root pane action uses only its owning root's
verified LIVE terminal. Without that live target the action is unavailable.
A terminal independently known to the app keeps its stock row. Sidebar root clicks use `agent.focus {terminal}`; graph root clicks
select and drill, double-click or Enter opens the root's pane per SPEC §6.6, and
graph agent clicks select and open. Pane-less transcript agents open panorama
Peek. Transcript references are inert data: pass as an argument to a read-only
viewer, never interpolate into a shell command. Peek gets title, status, model,
timestamps/duration, tool_calls, role, error, last_message, questions and the
owning root from the tables; a missing value stays unavailable.

Harness strings include claude, codex, gemini, cursor and amp; future strings
are valid. Only a missing or unknown harness may inherit along verified
parent/root ownership, never from cwd or appearance. Unsupported spelled
strings are preserved, never replaced by a parent harness. An unresolved
identity is the literal `unknown`; every serialized harness field is required.
Unsupported strings retain their spelling and use the app's generic tinted
`icons/agent.svg`; they never default to Claude. Matched stock rows use their
live `program.icon` and `program.tint`.

## Status, aggregation and questions

Agent codes: `q` queued/idle, `run` working, `rev` reviewing, `wait` needs an
answer, `blk` permission blocked, `fail` failed, `done` finished, `cxl`
cancelled, `inf` inferred ending, `lost` process disappeared, `bg` background
tasks. `stale` is a view-derived status, not a normal producer agent status;
accept it if received. Unknown codes remain visible with a generic `?` mark,
neutral colour and lowest urgency; they do not invent needs-you or done state.
For app marks use SPEC: spinner, » chip, ?N chip, ! chip, solid ✗ chip, ✓,
cancelled dash, queued/background ring. Codes not drawn in SPEC (`inf`, `lost`,
`stale`) use the existing model's `✓?`, `~`, `◌` with neutral colour.

The complete urgency order is the existing model's
`fail > blk > wait > rev > run > bg > q > lost > inf > done > cxl`, then
stale/unknown. It preserves SPEC's drawn-state order.

Root statuses are `working`, `blocked`, `idle`, `done`, `unknown` (future strings
use idle/unknown fallback). A node's own state is, for a root, its herdr status,
its pending prompts, the inbox questions no agent owns and its background work;
for an agent, its status. A subtree's needs marks aggregate over every
descendant at any depth. Display one chip for each needs kind present in the
subtree, ordered fail, blk, wait, then the root's own runtime mark. Own blocked
contributes the blocked chip once. A root belongs to Needs you if any agent is
fail/blk/wait, any open root inbox question exists, or its own status is
blocked; otherwise its own unseen done status places it in Done. An own idle
root with run/rev/working descendants belongs to Working; descendant completion
alone never places its parent in Done. Remaining roots follow their own
working/idle status. Pinning precedes these sections. A root/space working
indicator means any run/rev descendant or own working status. Listed native
descendants contribute urgency and chips even when displayed as independent
roots. Space totals count each entity's OWN aggregate in its proven space once;
display ancestry does not add that entity's totals to an ancestor a second time.

Finished agents are done/cxl/inf; lost/unknown must not silently disappear as
completed. Finished siblings fold into one `N finished · titles…` card or row per
parent at every depth, the root included. The writer's done window has already
dropped finished and failed agents older than `done_window` unless a shown
descendant needs them as an ancestor; `hidden_finished` counts them, and views
show it as `+N older` in the root's fold. A root never ages out.

`inbox`/`inbox_texts` is the complete root inventory; each pair of arrays has
equal length, unique ids and aligned texts. Agent question_ids/questions is an
ownership attribution of that inventory, not extra questions. Each agent pair
is also aligned and unique; each attributed id must exist in its root inventory
with the same text. An unattributed root question is the root's own, visible on
the root and in Peek context. Multiple agents can mention the same question;
union scoped ids for every subtree/root/space ?N. Never add root inbox length to
descendant question lengths. A wait agent with no inbox id still shows ?0;
pending text may explain it but creates no id. When inbox_ok is false,
inventory is unavailable (empty arrays), not evidence that previous questions
were answered; display the degraded/stale state.

Space needs-you item counts differ from ?N: count each fail/blk/wait agent
exactly once at any depth. Add exactly one root item when its own status is
blocked OR its inbox is nonempty, AND the detailed needs count is zero. This
conservative fallback avoids duplicate counts when ownership is not explicit;
never parse ids to classify ownership. The ?N union is independent of this count.
Root/space aggregates are derived in the mod after exact membership joins;
feed counts remain the totals above. Answers stay in the TUI inbox;
this feed grants no permission to answer, send text, or mutate a session. Graph
a/i actions navigate to the existing inbox/question group or focus the existing
permission pane as TUI actions do; they never answer, type, allow or deny.

Graph columns are depth columns: column 0 holds the roots, column n the
selected root's agents at depth n of its recorded spawn tree. The native
terminal tree merges into the same tree under its root with the existing
no-duplicate rule for a feed root that is also a native child; never assign
native children to a transcript agent from their position, title or other
guessed association.

## Executable fixtures

`tests/fixtures/app-feed/swarm.json` covers all agent statuses, root statuses,
mixed/unknown harnesses, a pane-less failure, repeated question attribution, and
no-transcript roots. Root `core:t2` holds 13 agents as one tree, four levels
deep, with a failed leaf at depth 3, a fold of three finished siblings (done,
cxl, inf) under one parent, and `hidden_finished` 2; root `root-0` has no agents
and its only problem is its own `blocked` status. `own-blocked.json` isolates a
blocked root whose agents are all healthy: it needs the operator once, with zero
failed agents. `degraded.json` exercises unavailable sources and errors as data.
Fixtures use synthetic identities and paths.

Run `cargo test --locked --test app_feed_contract`. The test reads these exact
field tables as its schema, checks every fixture recursively and checks the
totals against the existing Rust model. Negative mutations cover version,
required fields, types, duplicate identities and invalid question attribution.
`cargo test --locked --test app_feed_serialization` checks that the writer
reproduces each fixture whole from its model.
