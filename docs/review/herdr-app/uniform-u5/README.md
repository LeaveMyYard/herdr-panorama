# U5 evidence: the TUI dock and narrow views as one agent tree

These are renders of the Rust views (`render_dock`, `render_narrow`) from isolated synthetic or captured fixtures. No live herdr, app home or config was involved. A throwaway test dumped the cells, and they were drawn to PNG with rsvg-convert. The cell-for-cell assertions are the re-baselined goldens under `tests/fixtures/rust/screens/` (plan `docs/plans/uniform-agent-tree.md` §4, D5).

- [dock-46-synthetic.png](dock-46-synthetic.png): the 46-column docked panel over [fixture.json](fixture.json), a synthetic swarm.
  - Left, docked next to tab 1. The entry lists the root's own background task, then the two agent layers:
    - layer 1 is the root's direct subagents. A row's run is only `+N✓`, its finished children.
    - layer 2 is their subagents. A row's run shows all its descendants' states, most urgent first.
  - Nothing below layer 2 gets a row: `grep the harness icons` and `read the theme table` appear only in `Explore icon call sites`'s run.
  - The root's finished subagents fold into `✓ 3 finished · names`. `+2 older` counts the agents the done window dropped (`hidden_finished`).
  - Right, docked next to the Codex root. Its rows use the same component. The Claude entry, no longer docked, is two lines: line 2 has one tile per active direct subagent (its subtree's most urgent state), then `✓3`, then the counts.
- [dock-46-demo-before-after.png](dock-46-demo-before-after.png): the collected demo scenario. Before (U3) is the single adapter thread row `Billing API` with the thread's glyphs. After (U5) is the root's subagents as rows, plus the finished fold.
- [dock-46-deep.png](dock-46-deep.png): design 5d's deep dataset. `lead l1`, then `researcher r1` and `batch b1` with their whole subtrees as runs. No row goes deeper than layer 2.
- [dock-46-unseen.png](dock-46-unseen.png): design 9e's data with and without turn 6's unseen marks. An agent row whose agent or run holds an unseen event pulses its glyph's background (review round 1, M1), as does the finished fold.
- [narrow-70-synthetic.png](narrow-70-synthetic.png): the narrow layout at 70 columns over the same fixture.
  - The selected root opens one card per direct subagent. A card shows the agent's title, its subtree's marks, a pod per descendant, and its model and age.
  - Left: a depth-3 agent is selected. It is the bracketed pod in its depth-1 ancestor's card, which opens with its details.
  - Right: the root is selected, and its default subagent's card is open.
- [narrow-70-demo-before-after.png](narrow-70-demo-before-after.png): the demo capture. Before is Python's thread cards with phases. After is subagent cards and the finished fold.

Not in these renders: no orchestrator, thread or session rows; no phase lists; no `main:` or Codex session-root special cases.
