# U1a evidence: titles in the TUI

Renders of the Rust views over the captured demo scenario (`tests/fixtures/inputs/scenario-demo.json`,
collected in an isolated temp dir, no live herdr), at the scenario's own clock. Each image shows
the same swarm **before** (left: titles cleared, which is what a 1.3 core or the Python core
sends) and **after** (right: the U1a collector's titles). Cells were dumped from
`render_graph` / `render_narrow` by a throwaway test and drawn to PNG with rsvg-convert; the
cell-for-cell assertions live in `tests/app_screens.rs` (via `tests/harness_goldens/rows.json`)
and `crates/panorama-core/tests/card_title.rs`.

- [wide-folded.png](wide-folded.png): 200 columns. The folded depth card reads
  `Review PR #42 round 2` (the spawner's description) instead of `reviewer a4revie · opus`; its
  model moves to the bottom-border tags.
- [wide-selected.png](wide-selected.png): the same agent selected. Line 1 is its title and state;
  `reviewer a4revie · opus` becomes the first detail line (the card grows by one row).
- [narrow.png](narrow.png): 70 columns. The selected agent's row leads with the title, then
  `reviewer a4revie · opus · age · calls`.

Roots in this scenario keep their names: every demo tab label is operator-set, so the title
rule (§2.2) picks the tab, as before. The dock shows no agent rows for this scenario; its
spawn rows use the same title (checked in `card_title.rs`).
