# herdr-app feed v1

This is the Rust collector → herdr-app mod contract for the approved
[plan](../plans/herdr-app-mod.md), implementing the data needed by
[SPEC](../design/herdr-app/SPEC.md). It is a full snapshot, never a delta.
It extends `snapshot --json` (`src/snapshot/json.rs`) with `v`, orchestrator
`host` and `terminal`, and agent `harness`. It does not change that command or
the TUI panel protocol. A1 specifies the contract and fixtures; the writer and
host collection arrive in A2/A3.

## Delivery and validation

The writer publishes UTF-8 JSON at `<appHome>/data/panorama/feed.json`
(normally `~/.config/herdr-app/data/panorama/feed.json`). It writes a sibling
temporary file and renames it over the destination, so readers see complete
snapshots. `panorama app-feed --ensure` will start one detached watcher; repeated
calls are idempotent. The watcher recollects on change, with a slow polling
fallback, and exits after the app is no longer running. Precise watcher timing
and idle detection are A3 implementation choices, not new feed fields.

Readers require a JSON object and numeric `v` equal to 1. Equivalent JSON
spellings such as `1.0` and `1e0` are accepted. Missing version, `true`, another
version, missing required fields, and wrong types reject the snapshot.
All fields in the tables below are required, including nullable fields and empty
arrays. `?T` means T or null, `[T]` means an array, and object names refer to other
tables. `number` is finite POSIX seconds (fractional seconds allowed); `integer`
means a finite integral numeric value within ±9007199254740991 (JavaScript
`Number.isSafeInteger` semantics); decimal/exponent spellings are accepted. Strings are plain text, not markup or commands.
Unknown fields are ignored for additive v1 compatibility; changing a type or
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

## Feed

| Field | Type | Meaning |
|---|---|---|
| v | integer | Exactly 1. |
| generated_at | number | Collection time. |
| herdr_ok | bool | Selected host source was read successfully. |
| inbox_ok | bool | Open inbox questions were read successfully. |
| errors | [string] | Partial collection/source errors; empty on success. |
| counts | Counts | Existing snapshot totals, not app section/space totals. |
| orchestrators | [Orchestrator] | Visible roots; may have no transcript/threads. |

## Orchestrator

| Field | Type | Meaning |
|---|---|---|
| pane_id | string | Legacy source identity, nonempty; not necessarily an app pane. |
| host | string | `herdr-core`, `herdr-tui`, or `unknown`. |
| terminal | ?string | Exact core address, e.g. `t2` or `vps/t7`; null when unavailable. |
| harness | string | Harness identifier; unknown harnesses are retained. |
| tab_id | ?string | Legacy TUI tab id; core uses null. |
| tab_label | string | Legacy source title, not authoritative app tab/space name. |
| session_id | ?string | Verified harness session identity; never a join heuristic. |
| herdr_status | string | Root runtime status, vocabulary below. |
| workspace_id | ?string | Legacy TUI workspace id; core uses null. |
| focused | bool | Legacy focus observation; core uses false. App owns focus. |
| cwd | ?string | Source folder metadata; never identity or app membership. |
| flavour | string | Existing collector flavour, e.g. agents/workflow/pr-review. |
| transcript | ?string | Transcript reference/path, or null when unavailable. |
| pending | [string] | Root blocking prompts, including permission prompts. |
| inbox | [integer] | Open question ids for this root/session. |
| inbox_texts | [string] | Question text parallel to inbox, same order and length. |
| threads | [Thread] | Work threads in collector order. |
| hidden_done | integer | Nonnegative count of old completed threads omitted by collector. |
| tab_no | ?integer | Legacy 1-based TUI tab position; core uses null. |
| machine | ?string | Legacy source machine label, not app space membership. |
| machine_id | ?string | Legacy source machine identity. |
| bg | [Background] | Root background work still in flight. |
| last_activity | ?number | Newest transcript timestamp, or null. |

## Thread

| Field | Type | Meaning |
|---|---|---|
| id | string | Nonempty id unique within its root. |
| name | string | Thread label. |
| source | string | explicit/workflow/ref/turn/session, or unknown string. |
| phases | [Phase] | Ordered plan steps. |
| agents | [Agent] | Flat array, including descendants at every depth. |
| status | string | active/done/failed/cancelled, or unknown string. |
| started | ?number | Start time. |
| ended | ?number | End time. |
| worktree | ?string | Source metadata, never an identity join. |
| pct | ?integer | Existing collector progress percentage, normally 0–100. |
| flavour | ?string | Explicit thread flavour when known. |

## Agent

| Field | Type | Meaning |
|---|---|---|
| id | string | Nonempty agent identity within its root. |
| harness | string | Explicit harness or verified parent/root harness; else unknown. |
| description | string | Task description. |
| role | string | Display role. |
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
| phases | [integer] | Zero-based indexes into this thread's phases. |
| parent | ?string | Parent agent id in the same thread; null means thread child. |
| depth | integer | Legacy producer depth hint; reconstruct depth from parent links. |
| resumes | integer | Nonnegative resume count. |
| inferred | bool | Ending inferred rather than directly observed. |
| transcript | ?string | Transcript reference/path for read-only viewing. |
| seq | integer | Stable collector index, usable as deterministic tie-breaker. |
| bg | [Background] | Agent's background work still in flight. |

Additive titles (`docs/plans/uniform-agent-tree.md` §2.2, slice U1a). Writers from U1a on
also emit, on every Orchestrator and every Agent, `title` (string: the cleaned display name,
at most 80 characters) and `title_source` (string: which rule gave it; `pane`, `tab`,
`session`, `prompt` or `id` for a root, `spawner`, `prompt`, `type` or `id` for an agent;
unknown strings stay valid). They are not in the tables above because readers must not
require them: an older writer omits them, and an empty `title` means none was found (a
harness without title rules yet, or Claude's synthetic `main:` session agent). A reader
then falls back to `tab_label` or `role`. `title_source` drives no logic.

## Phase

| Field | Type | Meaning |
|---|---|---|
| name | string | Phase label. |
| state | string | done/now/next/failed/skipped, or unknown string. |
| start | ?number | Start time. |
| end | ?number | End time. |
| note | ?string | Current phase detail for graph and Peek context. |

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
| orchestrators | integer | Number of roots. |
| threads | integer | Number of included threads. |
| active_threads | integer | Threads other than done/cancelled (failed counts). |
| agents | integer | Number of flat agent entries, never recursively counted twice. |
| running | integer | Agents with run or rev status. |
| waiting | integer | Roots with nonempty bg plus agents with bg status. |
| questions | integer | Sum of root inbox lengths, not agent question lengths. |
| blocked | integer | Roots whose herdr_status is blocked. |
| failed | integer | Agents with fail status. |

## Identity, tree and app joins

Use `(host, terminal)` for a core root with a terminal, otherwise `(host,
pane_id)`. Every root key must be unique; duplicate enrichment roots reject the feed.
Both are opaque, exact strings. Thread keys append thread id; agent
keys append agent id to the root key. Agent ids may repeat across roots, but
must not repeat across threads of the same root. Thread ids may repeat across
roots. Question keys append question id to the root key and its session_id;
the same numeric question on another machine/session is not the same question.

For herdr-core, match `terminal` exactly against `agents().address` and
`terminals().address`; locate app panes by `panes().terminal`, then use live
app space/tab data. Never join using cwd, title, pane_id, session_id prefixes,
thread names, or guessed parentage. A TUI or unknown-host root cannot acquire
app membership from a coincidentally matching terminal string. Remote addresses
stay qualified: `vps/t7` does not match local `t7`.

Legacy membership fields remain only to preserve the snapshot shape. Core
writers use the placeholders documented above and add no space/tab membership
fields. App focus, pinning, changed age, section ordering, titles and icons come
from the matched stock agent; app space/tab membership always comes from live
app data. Every stock `agents()` row must remain present, including unsupported
harnesses and roots with no panorama transcript. Feed enrichment must never
filter those rows out. Unmatched feed roots remain in the sidebar under their opaque feed identity,
using supplied tab_label, pane_id, harness and status as data. Never invent an
app space/tab/name/section/membership/focus for them, or portray stale/degraded
feed provenance as fresh live app evidence. They are excluded from space/graph
aggregates; a space graph contains only roots with a verified live membership join.

Native pane-less terminals may inherit membership only through a verified LIVE
terminals().parent chain to an exactly joined live stock ancestor. Direct child
pane/stock membership wins. Each agents() entry stays a top-level orchestrator,
including descendants with their own pane. Native sidebar recursion includes
only children absent from agents(), stops at listed-root boundaries, and gives
each root its own subtree. Preserve stock descendant urgency and count each
canonical native terminal identity once. Transcript agents remain separate;
never infer their address or membership from ids, cwd, titles or session names.

Parents define an arbitrary-depth tree within a thread. Never truncate at a fixed
depth. Render dangling/self-parent entries as thread children. As in
`TreeKids::of`, entries unreachable from any root because of cycles become
thread children, preserving every agent exactly once. Do not use `depth` to
guess missing parents. Retain producer order among siblings, with `seq` as a
stable tie-breaker where a view sorts.

Agent entries do not carry terminal addresses in v1. Detailed subagents are
therefore pane-less; do not interpret their ids as terminal addresses. Even a synthetic `main:*` id is not terminal identity proof. Every detailed
agent opens Peek; its Open orchestrator action uses only its owning root's
verified LIVE terminal. Without that live target Open orchestrator is unavailable.
A terminal independently known to the app keeps its stock row. Sidebar root clicks use `agent.focus {terminal}`; graph root/thread clicks
select and drill, double-click or Enter opens the owning orchestrator per
SPEC §6.6, and graph agent clicks select and open. Pane-less transcript agents
open panorama Peek. Transcript references are inert data: pass as an argument
to a read-only viewer, never interpolate into a shell command. Peek gets status,
model, timestamps/duration, tool_calls, role, error, last_message, questions,
thread name and owning root from the tables; a missing value stays unavailable.

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
stale/unknown. It preserves SPEC's drawn-state order. A thread mark is the most
urgent agent anywhere in its flat array; do not treat thread status as an agent
code. Empty threads fall back to done for done, dash for cancelled, ✗ for failed,
ring otherwise. Unknown phase state uses the next/skipped neutral bar.

Root statuses are `working`, `blocked`, `idle`, `done`, `unknown` (future strings
use idle/unknown fallback). Display one chip for each needs kind present in the
subtree, including the fallback fail mark of an EMPTY failed thread, ordered
fail, blk, wait, then the root's own runtime mark. Own blocked
contributes the blocked chip once. A root belongs to Needs you if any agent is
fail/blk/wait, any empty failed thread exists, any open root inbox question
exists, or its own status is blocked;
otherwise its own unseen done status places it in Done. An own idle root with
run/rev/working descendants belongs to Working; descendant completion alone never
places its parent in Done. Remaining roots follow their own working/idle status.
Pinning precedes these sections. A root/space working indicator means any run/rev
descendant or own working status. Listed native descendants contribute urgency
and chips even when displayed as independent roots. Space totals count each
entity's OWN aggregate in its proven space once; display ancestry does not add
that entity's totals to an ancestor a second time. Finished folding follows thread done/cancelled and agent
done/cxl/inf states; lost/unknown must not silently disappear as completed.

`inbox`/`inbox_texts` is the complete root inventory; each pair of arrays has
equal length, unique ids and aligned texts. Agent question_ids/questions is an
ownership attribution of that inventory, not extra questions. Each agent pair
is also aligned and unique; each attributed id must exist in its root inventory
with the same text. An unattributed root question is still visible on the root
and in Peek context (a synthetic main agent may own it). Multiple agents can
mention the same question; union scoped ids for every thread/root/space ?N.
Never add root inbox length to descendant question lengths. A wait agent with
no inbox id still shows ?0; pending text may explain it but creates no id.
When inbox_ok is false, inventory is unavailable (empty arrays), not evidence
that previous questions were answered; display the degraded/stale state.

Space needs-you item counts differ from ?N: count each fail/blk/wait agent
exactly once at any depth. Add exactly one root fallback item when its own status is blocked OR its inbox
is nonempty OR any empty failed thread exists, AND the detailed needs count is zero. This conservative fallback
avoids duplicate counts when the representation is not explicit; never parse
`main:*` ids to classify ownership. The ?N union is independent of this count.
Root/space aggregates are derived in the mod after exact membership joins;
feed counts remain the legacy totals above. Answers stay in the TUI inbox;
this feed grants no permission to answer, send text, or mutate a session. Graph
a/i actions navigate to the existing inbox/question group or focus the existing
permission pane as TUI actions do; they never answer, type, allow or deny.

Graph transcript depth columns contain only the selected thread's recorded spawn
tree. Use the native terminal tree as a fallback only when no transcript thread
exists; never invent a thread label or assign native children to a transcript
thread from their position, title or other guessed association.

## Executable fixtures

`tests/fixtures/app-feed/swarm.json` covers all agent statuses, root statuses,
phase/thread/source vocabularies, four nested agent levels, mixed/unknown
harnesses, a pane-less failure, repeated question attribution, and no-transcript
roots. `degraded.json` exercises unavailable sources and errors as data.
`empty-failed.json` isolates an empty failed thread under a working root: it
needs the operator and retains the root spinner, while legacy failed-agent
count remains zero.
Fixtures use synthetic identities and paths.

Run `cargo test --locked --test app_feed_contract`. The test reads these exact
field tables as its schema, checks every fixture recursively and checks the
legacy totals against the existing Rust model. Negative mutations cover version,
required fields, types, duplicate identities and invalid question attribution.
This validates the contract; it does not claim a writer or app runtime exists.
