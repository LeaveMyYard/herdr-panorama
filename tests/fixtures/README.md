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

- `graph_rules.json`: Python's wide-graph columns (`render.orch_column`, `thread_column`,
  `agent_card`) on a small swarm built for their rules (the swarm is in the file, in wire form,
  with the view's unseen marks), each canvas as runs of `[text, fg, bg, bold]` per row; the
  golden for `crates/panorama-core/tests/graph_columns.rs`. Captured by
  `uv run python tests/fixtures/capture_graph_rules.py > tests/fixtures/graph_rules.json`.

Behaviour goldens (`docs/plans/rust-rewrite.md` §5.2, items 2 and 3). They were captured from the
running Python code by `tools/capture_goldens.py`, and none of them is written by hand.
Anchor: the Python bridge release on main at merge commit `b142c7b` (0.1.1, PR #56), plus the
merge commit of PR #57, which added them. This repo uses no git tags. Every golden carries its
inputs, so another implementation can replay it without Python:

- `inputs/scenario-*.json`: each `tests/scenario.py` scenario (`SCENARIOS`) as data:
  - the files, each with its path relative to the scenario root, its mtime (a transcript's
    newest record time) and its text;
  - herdr's snapshot;
  - the inbox rows (herdr-inbox's 0.1 `questions` table).
  Record uuids and tool ids are derived from the record's position, so they are the same on
  every run.
- `inputs/swarm-*.json`: the model-built swarms (`tests/demo.py`, the design datasets) in their
  wire form (`protocol.swarm_to_wire`), with the clock their tests use.
- `model/*.json` (model goldens): `panorama snapshot --json`, run as the real command in its own
  process with `time.time` held.
  - Inputs: the scenario materialised under `<root>`: `CLAUDE_CONFIG_DIR=<root>`, the inbox
    database, the events dir, and a fake herdr whose `api snapshot` is the input's snapshot and
    whose other calls answer `ok`.
  - Times: the scenario's `now` plus each key's minutes.
  - Conditions: herdr down and no inbox, as named in `conditions`.
  - Paths under the root read `<root>`.
- `screens/*.json` (app-screen goldens): the Textual apps run headless (`run_test`,
  `notifications=True`): `SwarmApp` as the popup, the home tab and the docked panel;
  `NewAgentApp`; `PanelApp`.
  - Each file has its `runs`: the app, size, input, held `now`, modes and env.
  - Each run has an ordered `steps` list, explained in `screen_files`' docstring. Each `snap`
    step names a screen in `screens`.
  - A screen is what Textual's compositor sends to the terminal, as runs of
    `[text, fg, bg, bold]` per row: the shape of `design_*.json`, with `null` for the
    terminal's default colour.
  - Reverse video is resolved: a cell with Rich's `reverse` (the text cursor in the New agent
    form) is stored with its fg and bg swapped, as the terminal shows it. No other attribute
    (dim, italic, underline, strike, blink) occurs on these screens, checked over every
    segment of every capture, so the format stays `[text, fg, bg, bold]`. herdr-inbox's
    goldens use the same rule.
  - Fixed: TZ=UTC, panorama's default theme, animation frame 0 unless a screen's name says
    otherwise.
  - `unseen`: the unseen marks (agent id → `fail`, `wait` or `done`) the view drew each snap
    with, by the snap's name. They are an input, not part of the screen: the tracker's state
    file is shared by every run of one capture, so a run's marks depend on the runs before it
    (agents of different inputs share ids). A port renders a snap with them.

How to compare, for a port:

- **Model files**: compare as JSON values. Key order and float formatting (`1791972600.0`) are
  not part of the contract.
- **Screens**: compare cell by cell. Split each run into one cell per character, with `null`
  for the default colour. Where one run ends and the next begins comes from Rich's segments,
  so run boundaries are not part of the contract.

`tests/test_goldens.py` captures everything again into a temporary directory and compares, so a
behaviour change fails it. After a deliberate change, regenerate and say why in the PR:

    uv run python tools/capture_goldens.py

Not covered yet (#58): a non-empty `errors` list, scrolling, mouse targets, the answer/inbox
flows, `is_stale` exactly at `stale_after`, a hidden per-tab panel (by design it keeps its last
picture), and remote machines in the model (`panorama snapshot` reads only this machine). The
inbox popup's screens (§5.2 item 2) belong to the herdr-inbox repo.
