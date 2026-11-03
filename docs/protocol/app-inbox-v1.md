# Private app Inbox navigation v1

The installer-owned `data/panorama/bin/panorama` supplies optional navigation to
an existing enabled `leavemyyard.herdr-inbox` plugin. This does not install or set
up Inbox, change the TUI plugin configuration, answer questions, type text,
approve permissions, or expose inbox contents. The ordinary TUI `InboxLink`
runner and its existing fallback behavior are unchanged.

| Invocation | Success stdout | Exit behavior |
|---|---|---|
| `app-inbox --help` | Constant text beginning `panorama app-inbox v1` | 0, no filesystem/RPC/helper activity. |
| `app-inbox --available --app-home ABSOLUTE_HOME [--install-id GENERATION]` | Exactly `{available:boolean,questions:boolean}` as JSON | 0 for a valid private installation, including unavailable `false,false`. |
| `app-inbox --open [--question ID] --app-home ABSOLUTE_HOME [--install-id GENERATION]` | Exactly `{"opened":true}` | 0 only when the existing installed CLI returns success. |

`questions` is a capability boolean: the installed `open --help` advertises
`--question`. It is never a count, array, or question query. `available` requires
an enabled plugin registration with a nonempty absolute root and a successful
help run through that root's existing regular executable `.venv/bin/inbox`.
Missing executable, disabled/missing plugin, failed help, or unsupported optional
integration becomes unavailable. Only a successful plugin lookup can establish
absent/disabled registration; lookup transport errors, refusals and malformed
replies are operation errors. A command timeout is an error, not availability
proof. `questions` is false whenever `available` is false.

Strict lookup validates the raw result object and its required `plugins` array.
Every entry must be an object with a nonempty string `plugin_id`, string
`plugin_root` and boolean `enabled`; plugin identities must be unique. These are
the identity/availability fields of the existing registered-plugin response;
other metadata is additive and ignored. No malformed entry is silently skipped.
A valid empty array or disabled Inbox registration is ordinary unavailable data.
An empty string root cannot provide the installed entrypoint and is unavailable.
The existing kit/TUI lenient decoder is unchanged.

Optional `--install-id` pins the operation to a 32-lower-case-hex generation.
Malformed or duplicate generation arguments exit 2 before I/O. A supplied value
that differs from the validated v2 receipt exits 1 before any TUI RPC or helper.
Omission is manual invocation targeting the current receipt; after initial proof,
that operation also retains its verified generation through the final pre-open
check. Frontend clients MUST supply their initially pinned generation on each
non-help call, re-read receipt/artifact each time, and permanently stop on a
changed/missing receipt as specified in the runtime protocol.

`ID` is canonical ASCII decimal in [1, 9007199254740991], without leading zero,
sign, prefix, exponent or fraction. Unknown/duplicate/malformed arguments or IDs
exit 2 with usage on stderr and no I/O. Invalid/missing/disabled installer receipt,
private artifact mismatch, timeout, unavailable open, or navigation failure exits
1 with a diagnostic on stderr and no stdout. Empty stdout never means success.

Every non-help command shares one five-second total budget, including lifecycle
lock acquisition, full private artifact hashing, plugin lookup, help and open.
Hashing and lock polling check the remaining budget. The internal operation
reserves cleanup time within that budget. A timed-out owned helper is cancelled
and reaped through its own child handle; recorded agent/server/watcher PIDs are
never signal targets. There is no retry after timeout or an ambiguous failure.
The caller disables duplicate dispatch until its pending invocation settles.

Before any TUI call, validate the v2 receipt, exact current private executable
path/hash/0755 mode and enabled generation control under the app-home directory
FD lifecycle lock described in [the runtime protocol](app-feed-runtime-v1.md).
Release the lock before external lookup/help. For open, revalidate receipt,
artifact and enabled control immediately after capability lookup and before the
actual navigation call. A running writer is not required. The same deadline
covers both validations. This is a validation boundary, not a claim to prevent
arbitrary concurrent modification after the check.

The strict runner executes only the enabled registered plugin's existing
`.venv/bin/inbox`, from its root. It never runs `uv`, installs dependencies,
resolves `inbox` on PATH, invokes the opaque registered action, or falls back to
plain open. The permitted argv are:

- Capability query: `open --help`.
- Plain navigation: `open`, adding `--no-hold` only when advertised.
- Question navigation: `open --question Q<ID>`, adding `--no-hold` only when
  advertised. If question capability is absent, fail without plain fallback.

Strip all inherited `HC_*`, `HERDR_*` except the existing TUI socket transport,
`UV_*`, `VIRTUAL_ENV*`, `INBOX_*` and `PYTHON*`; set only the registered Inbox
plugin ID/root and `PYTHONDONTWRITEBYTECODE=1`. No caller pane is forwarded.
Python bytecode cache files must not be created. The runner accepts no pane,
session, answer, dismiss, reply, allow/deny, delivery, installation or setup args.

Success means the existing CLI accepted navigation: its popup-open RPC returned
success. It does not prove the popup was rendered, focused on a still-present
question, or subsequently closed. The installed primary source maps the `inbox`
entrypoint in `herdr-inbox/pyproject.toml` to `herdr_inbox.cli:main`; the `open`
branch in `src/herdr_inbox/cli.py` writes an optional one-shot focus request and
returns the result of `herdr.open_popup` (`src/herdr_inbox/herdr.py`). `--no-hold`
changes popup arming, not completion waiting. The existing CLI rejects an
already-running question popup rather than opening a second one; the bridge
never retries that rejection. No live Inbox navigation was used to establish
this protocol. Tests use a fake TUI server and fake installed CLI exclusively.
