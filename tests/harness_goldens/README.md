# Rust harness identification deltas

The operator approved stable existing harness names in the graph, narrow list, and dock;
Codex is also enabled in the existing New agent carousel. Python behavior and original
Python/design captures remain unchanged.

`rows.json` holds fixed expected cell spans, scoped to exact fixture/state/row paths.
Each entry checks the original row's normalized FNV-1a digest and every replaced cell;
replacement spans have the same width. Cells outside those spans still come directly from
the original capture and are compared exactly. These expectations are committed data:
there is no update-from-renderer switch or ignored mismatch in the test helper.

Codex's existing carousel art/name become enabled and its Coming soon badge disappears
(including its neighboring ghost). The image shows the original capture on the left and the
exact Rust expectation on the right at identical cell dimensions. The cell-for-cell Rust tests
verify the right side:

- [Codex launcher](evidence/launcher.png)

The rows for the harness names in the dock, the narrow list and the wide graph were retired in
U6 (plan `docs/plans/uniform-agent-tree.md`). Since U4 and U5 those screens are Rust-owned
goldens (`tests/fixtures/rust/screens/`, D5), so no test compared the adapted Python and design
captures any more. Their rows and evidence images are in the git history.

New mixed-harness tests separately assert both labels, exact pane hit targets, navigation,
filters, and that bare Codex idle/unknown does not appear completed. Legacy Claude's
empty-session graph behavior is retained for compatibility.
