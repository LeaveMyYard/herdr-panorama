# B3 author evidence

Runtime source: `92babb4ccadc28397674d8738cb6611252cd1fda`, after actual B2 merge `96ff7ae` / reviewed `87ec111`. `FINAL-SOURCE.json` records exact production JS hashes and cleanup. Later evidence/document commits do not change those runtime bytes. Review and operator installed visual acceptance remain pending.

## Design comparison

Design left / native app right, both 1920×1080 at app zoom 1.2:

- [D comparison](compare-d.png), [native D](final-renderer-d.png), [approved D](../../../design/herdr-app/v2-d-graph-tab.png).
- [D2 comparison](compare-d2.png), [native D2](final-renderer-d2.png), [approved D2](../../../design/herdr-app/v2-d2-graph-scrolled.png).

These are **synthetic design-data renderer captures**, not fresh writer evidence. The existing capture method translates the approved HTML's SW2/T144/DEEP dataset into v1 feed fields. Synthetic reporter terminals are created through the isolated core before app launch, then kept into exact Spaces. Wrapper subclasses only seed the fixture and selected thread/agent; production rendering/geometry remains unchanged. No receipt exists in this capture home, so canonical clients fail closed and the fixture stays stale; no private helper or Inbox navigation executes. The production sidebar is included.

The author compared fixed widths/window placement, four visible columns, thread details, focused checker/question/actions, parent connectors, folded siblings, breadcrumb, thumb, counts and Swarm glyph/header against D/D2. Functional data differences: the translated complete dataset contains 24 agents versus the reference's static 21; native ages/tab positions/status acknowledgement and stock repo/git details reflect the isolated core/worktree; phase timing absent from the fixture is not fabricated. Answer is disabled because this synthetic capture has no verified installed Inbox capability. Shipped frame chrome/update toast follows the actual installed app. These facts are not a visual acceptance verdict; the independent reviewer captures separately and records any remaining deviation.

The failed empty-agent attempt is preserved as `failed-empty-agent-capture-{d,d2}.png`: `space.new`/`tab.new` capture argv produced zsh terminals with null status/app, so exact stock membership correctly refused Space assignment. A later `terminal.keep` before the native model had observed the reporter also refused. The corrected capture reuses the successful interop ordering (reporters before app snapshot), rather than guessing membership or changing product code. `final-renderer-{core,app,status}.json` records the final actual model/status; errors are empty.

## Actual current-generation manual smoke

[current-generation-manual.json](current-generation-manual.json), [initial native window](current-generation-native.png), [reinstalled native window](reinstalled-native.png) are a separate bounded manual pass using the actual optimized reviewed artifact (9,964,424 bytes, SHA256 `da6d69e802a479d47aa154d3e6464c7286d0c06ef25304c9ca5a22e0aec328f2`). It was copied into a B3-owned artifact path, then installed through the real six-mod installer into a private HOME/app/core/session/XDG/socket tree. Core environment, zero initial terminals and private sockets were verified before structural commands.

The actual writer published the synthetic native root's working→blocked change. The frontend had fresh, validated current-G packets before uninstall and after reinstall under a different G; graph showed its no-transcript native root, tab/header needs count and stock Files panel. This proves basic initialization/native-root rendering, not a rich transcript tree or native-child-depth scenario. No transcript session was guessed.

Panel selection was written by the real stock-derived tabs behavior after native layout commands. Spaces folding/recovery fixture values were seeded only in this owned temporary store and consumed read-only; no machine connection was invoked. Both exact mode-0600 stores survived uninstall/reinstall and reopen byte-for-byte. Neither preexisting preference parent entered the replacement receipt's `dirs`. No graph store appeared. Active uninstall cleared only the two allowed null shared slots, the real writer exited cooperatively, stale-G startup refused, and original personal fixture bytes/modes remained. No Inbox open/answer/text/config action was invoked.

Both app/core stacks stopped before scratch removal; final owned-process scan is empty and `runtime.lock` is free. The operator's app/config/layout were unchanged.

## Existing checks

- `scripts/app-build --check`: canonical generated closure matches.
- `node --test app/tests/*.test.mjs`: 174 passed, 0 failed/skipped; the original 35 B3 tests remain.
- `scripts/app-check`: all six actual native mod checks passed in private HOME under `runtime.lock`.
- `cargo fmt --all --check` and existing `app_feed_protocol::shared_lifecycle_fixture_parses_exact_fields_and_hashes`: passed in a private B3 build target. Rust change only updates the existing expected reserved-directory list from three to five.
- Installer Python AST, finite JSON documents and `git diff --check`: passed.

No new automated feature cases/matrices were added after the operator's testing-policy change. Existing tests received only factual six-mod/metadata/v1-fixture/approved Spaces reuse updates. Deferred followups: [rich assembled E2E #143](https://github.com/LeaveMyYard/herdr-panorama/issues/143), [Peek/transcript #144](https://github.com/LeaveMyYard/herdr-panorama/issues/144), [preference/reinstall automation #145](https://github.com/LeaveMyYard/herdr-panorama/issues/145), [extended native input/visual/performance #146](https://github.com/LeaveMyYard/herdr-panorama/issues/146).
