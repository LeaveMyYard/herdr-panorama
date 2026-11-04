# U4 evidence: the TUI graph by depth

These are renders of the Rust wide graph (`render_graph`) after U4 (plan
`docs/plans/uniform-agent-tree.md` §4, D5). The columns are depths: the roots are depth 0,
then the selected root's agent tree, depth by depth. There is no orchestrator column and no
thread column. Every depth draws the same card: pod, title, the subtree's need marks (✗ ! ?N),
then its state or what it does. Finished siblings fold into one `N finished · names` card
per parent, at every depth. The root's own group also counts the agents the done window
hid (`+N older`).

The swarm is synthetic (`tests/fixtures/rust/inputs/swarm-uniform.json`): no live herdr, no
personal data. Each image is a screen of `tests/fixtures/rust/screens/uniform-wide.json`, the
Rust-owned golden that `crates/panorama-core/tests/wide_screens.rs`
(`the_uniform_tree_at_160_and_240_columns`) compares cell for cell. They were drawn to PNG
with rsvg-convert. The same screens as text are in [renders.txt](renders.txt).

160 columns (the roots' column is 42 wide, depth columns 44):
- [160-billing-api-the-root.png](160-billing-api-the-root.png): the keyboard is on the root, so
  its card unfolds with its own state. It shows the question no agent filed (`? Merge PR #41…`;
  U3 lost the "waiting on you" card that used to show this) and the root's own `g`/`t`/`a`
  hints, which are the only hints on screen because the keys act on the root. The agent
  remembered below it stays folded. Depth 1 shows the root's agents, with their fold
  `3 finished · +2 older`. Depth 2 shows the reviewer's children with their finished fold.
- [160-billing-api-depth-2.png](160-billing-api-depth-2.png): the keyboard at depth 2. The
  root folds back to four rows (`tab 1 · age` on its border). The selected card unfolds, with
  `role id · model` on its `agent` line.
- [160-release-notes-all-finished.png](160-release-notes-all-finished.png): a root with no
  agent left in the window shows `no subagents in the last hours`, and its card says
  `+3 older finished, hidden`.

240 columns (roots 44, depth 56):
- [240-search-refactor-depth-2.png](240-search-refactor-depth-2.png): a Codex root with the
  keyboard on `Trace the manifest cache` at depth 2. Every card carries its subtree's marks
  (`✗ ?1`), the last column shown shrinks to fit, and the line above the columns names the
  depths off to the right.
- [240-search-refactor-depth-5.png](240-search-refactor-depth-5.png): the keyboard at depth 5.
  The roots scroll off to the left, and the breadcrumbs are titles.
- [240-docs-site-the-root.png](240-docs-site-the-root.png): a blocked root's unfolded card shows
  the prompt it blocks on and its background task (where the `bg:` card used to be). Its
  `a answer` hint goes to the pane, but `answer in pane` does not fit the 44-column card,
  so it shortens to `answer`, as an agent's does.
