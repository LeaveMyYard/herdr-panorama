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
    herdr api snapshot | uv run python tests/fixtures/capture.py --snapshot > tests/fixtures/herdr_snapshot.json

Design references (not wire formats), both captured from the design's own code in
`docs/design/`, never written by hand:

- `design_data.json`: the design's sample dataset (`DATA` in `swarm-tui.js`), the shape
  `tests/design_dock.py` turns into a model Swarm.
- `design_3a.json`: the design's render of the docked panel 3a (`render-3a.mjs`, frames 0–4
  and 7), as runs of `[text, fg, bg, bold]` per row; the golden for `tests/test_dock_panel.py`.
Regenerate with `uv run python tests/fixtures/capture_design.py` (needs node).
