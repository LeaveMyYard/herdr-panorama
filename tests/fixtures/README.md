# Test fixtures

- `templates.json`: one real record per kind herdr-panorama reads (main and
  subagent transcript records, task notifications, agent meta files, workflow
  journal and run summary), captured from real Claude Code 2.1.x files by
  `capture.py` and anonymised: keys, key order, nesting, value types and the
  structural values the parser keys on are kept; every id, timestamp, path and
  piece of text is replaced. Included: every carrier of a `<task-notification>`
  (queue-operation enqueue and queued_command attachment in the main
  transcript, queued_command attachment in an agent's file, user string), the
  SendMessage resume result (`resumedAgentId`), foreground and background
  `Agent` results, nested and fork metas, human-origin prompts, and the
  `custom-title` / `agent-name` / `ai-title` records.
- `herdr_snapshot.json`: `herdr api snapshot` (herdr 0.9.3), one anonymised
  entry per list, captured the same way (`capture.py --snapshot`).

`tests/scenario.py` builds test project dirs only from these templates, so the
wire format is captured, never invented. Regenerate on a machine with real
transcripts:

    uv run python tests/fixtures/capture.py ~/.claude/projects > tests/fixtures/templates.json
    # or add only the kinds a new feature needs, keeping every existing template as it is:
    uv run python tests/fixtures/capture.py ~/.claude/projects tests/fixtures/templates.json > new.json

The background-work kinds (design turn 8: background Bash and its timed-out variant, Monitor,
TaskStop, the orphan-summary notice, Monitor events and expiry) were added that way. In a
notice only the tag skeleton is kept, except the values the parser keys on: the
`__orphan_summary__` marker id and the `[Monitor expired` event prefix.
    herdr api snapshot | uv run python tests/fixtures/capture.py --snapshot > tests/fixtures/herdr_snapshot.json

Design references (not wire formats), both captured from the design's own code in
`docs/design/`, never written by hand:

- `design_data.json`: the design's sample dataset (`DATA` in `swarm-tui.js`), the shape
  `tests/design_dock.py` turns into a model Swarm.
- `design_9.json`: the design's render of the docked panel 9e (`render-9.mjs`, eight frames,
  plain and with turn 6's unseen marks) and its dataset, as runs of `[text, fg, bg, bold]` per
  row; the golden for `tests/test_tiles.py`.
- `design_8.json`: turn 8 (waiting and stale, `swarm-states.js` via `render-8.mjs`): its added data
  and frames 0, 4, 7, 12 and the fractional 1.5 and 3.4 (the eased blink) of the 2a graphs and the 9e panel; the golden for `tests/test_states.py`.
- `design_10.json`: turn 10, the 9e panel focused and not in two herdr themes (`render-10.mjs`,
  four frames) and the theme tokens used; the golden for `tests/test_focus_theme.py`.
Regenerate with `uv run python tests/fixtures/capture_design.py` (needs node).
