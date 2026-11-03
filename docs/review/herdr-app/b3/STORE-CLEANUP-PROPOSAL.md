# B3 preference stores: observed lifecycle and proposed finite cleanup

**Superseded:** the operator chose to retain exactly `data/panorama-spaces/store.json` and `data/panorama-tabs/store.json` with their necessary parents. Stock panel/folding/machine-reconnection persistence stays. The empty-store deletion/cohort proposal below is historical evidence and will not be implemented. Reinstall validation/preservation is tracked in [#145](https://github.com/LeaveMyYard/herdr-panorama/issues/145); extended native input/visual checks are [#146](https://github.com/LeaveMyYard/herdr-panorama/issues/146).

Original status: proposal for the root's later explicit inventory/lifecycle handoff. Only graph age persistence has been removed from product code. Tabs and Spaces retain their cloned stock preference persistence. No store deletion, generation marker, shared key, installer change, or cleanup callback is implemented by this proposal.

## Actual owned-host evidence

The probe used herdr-app 0.1.12 and hc 0.1.17, an owned temporary `panorama-store-probe` module, a separate core identity/PID with zero terminals, task-owned app/XDG/socket paths under the user's home, and the common runtime lock. It used the real `localStorage` APIs; it never edited a personal store or invoked a machine connection. Raw results, including exact bytes, are in `store-api-probe.json`. The restart test reopens the same owned app home in read-only mode: it does not reseed values on reopen.

| Operation | Actual result |
|---|---|
| Never access storage, or `getItem` only | No `store.json` file. |
| Set the three synthetic stock preference values; `flush` after mutations | Regular mode-0600 JSON object; every outer value is a string. |
| Close app, reopen, read only | All three preference values and file bytes survive exactly. |
| `removeItem` for the known keys; `flush` after mutations | File remains, exact bytes `{}`. |
| Seed `personal.extra` plus `panels`, then remove only `panels` | Unknown `personal.extra` survives. |
| `clear`; `flush` after mutations | File remains, exact bytes `{}`. |
| Close the view and app after clear | The same empty file remains. |
| `flush` with no preceding mutation | Did not complete within the probe's 300 ms bound, both with no file and with an already empty file. This is not a successful durability acknowledgement. |

The empty file is two bytes, has no trailing newline, and has SHA-256 `44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a`. Supported API clearing alone does not meet no-trace cleanup. Production must never blanket-clear a store containing unknown keys. Normal app/view close must continue to preserve stock preferences.

Graph ages are now a plain per-View `changed` object: no `localStorage` read or write. There is no reason to authorize a graph age-map file or to migrate/adopt a preexisting graph store.

## Finite proposed ownership

Only these two host-created preference files need a new explicit cleanup rule:

| Exact path, relative to app home | Owned keys |
|---|---|
| `data/panorama-tabs/store.json` | `panels` |
| `data/panorama-spaces/store.json` | `machines.folded`, `machines.reconnecting` |

The current empty-directory-only allowance cannot remove either file, even if empty. The proposed authority is tied to the current validated v2 generation G: the receipt must record the exact parent directory as freshly created for that install, before loading the mod. A preexisting data directory or store is never adopted, even if empty. In particular, the root must explicitly transfer the new Tabs data-directory reservation and the two file rules with the later B3 inventory handoff. No new runtime file, shared key, generation marker, or receipt schema field is proposed; existing receipt G plus fresh-directory ownership supplies provenance.

## Strict producer schemas before any preference removal

The host envelope must be an object with string values, with no duplicate keys. Only the named keys for that exact mod are eligible. Missing keys are fine. Unknown keys, malformed values, unsafe files, or newer unsupported fields preserve/report the store and refuse automatic cleanup; they are not silently rewritten or treated as owned.

- `panels` is a JSON object mapping native space IDs to `null` or a string panel ID. IDs stay opaque: the stock clone supports custom enabled panels, so cleanup must not invent a `files`/`changes`-only restriction.
- `machines.folded` is a JSON array of unique `null` or string machine identifiers, matching the clone's Set serialization.
- `machines.reconnecting` is a JSON object keyed by machine name. Each value has exactly `machine`, `removed`, `busy`, and `error`. `removed` and `busy` are booleans; `error` is null or a string. `machine.name` matches the outer key. The `machine` value follows the installed hc `MachineView` schema, captured in `store-machine-schema.json`: required string fields `name`, `target`, `id`, `host`, `version`, `state`; optional `session` and `why` are string/null, `since_ms` is nonnegative safe integer/null, and `via`/`temporary` are booleans. Cleanup should reject extra fields it does not understand. Machine targets are inert data, never commands to run during cleanup.

This is a proposed closed cleanup schema for the current shipped clone, not a claim that the hc API itself forbids future additive fields. Future/unknown shapes are preserved rather than deleted.

## Proposed uninstall sequence and remaining handshake gap

1. Keep normal stock reads/writes and app-restart persistence. Only an uninstall transition proven to belong to current G may begin cleanup. Missing/changed receipt, stale data, or an arbitrary `client.stopped` flag is not sufficient authority to clear another generation's preferences.
2. On validated current-G disable, freeze further preference writes for every active owned Tabs/Spaces instance. Validate the exact known shapes, remove only their owned keys through `localStorage.removeItem`, and obtain a positive durable-clear acknowledgement after actual mutations. No-op `flush` timeout must never count as success. Ordinary close keeps preferences.
3. The canonical lifecycle must coordinate this durable preference cleanup with its frontend acknowledgement. Its existing shared-cache token release alone does not prove that every preference writer has stopped or flushed. This requires a narrow root/B2-agreed lifecycle seam; B3 is not inventing a callback or shared protocol before that handoff.
4. After writer and all relevant frontend preference activity are quiescent, the installer revalidates receipt G, disabled control, and its created-directory ownership. Only the exact file at either allowlisted path may be unlinked, and only when it is a regular single-link mode-0600 file with exact bytes `{}`. Use the installer's no-follow directory-FD discipline and verify the current path/inode/content immediately before removal. A missing file is already clean. Changed bytes, unsafe replacements or unknown content preserve/refuse; no recursive removal or arbitrary JSON rewrite.
5. Remove only empty owned parent directories using the existing rmdir rule. Preserve receipt/disabled state and refuse completion on timeout or ambiguity. Retry is idempotent and must never re-arm a stopped writer or preference writer.

This sequence needs explicit root approval of finite paths/ownership and agreement on the frontend durability seam before implementation. It does not alter the signed A3 writer now. The only accepted post-uninstall shared residue remains `panorama.feed:null` and `panorama.feed.owner:null`; an empty `store.json` is not another exception.

Required later proof: stock restart/restore parity, active uninstall, ordinary close without preference loss, interrupted cleanup/retry, multiple active caller instances, old/new-G race, unknown keys/fields, unsafe replacements, and actual no-trace readback after a used temporary-home uninstall. Final integration still waits for B2 merge, latest canonical regeneration and the root's layout/inventory handoff.
