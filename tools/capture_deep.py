"""Capture test_deep's collector scenario (the Python test's own builder) as a Rust input:
the files with their mtimes, a herdr snapshot naming the session, and the times.

    uv run python tools/capture_deep.py . <empty scratch dir> > tests/fixtures/collect/deep.json

Read by the Rust collector's tests (tests/collect_deep.rs). Kept out of tests/fixtures/inputs/,
which tools/capture_goldens.py owns."""
import json
import os
import pathlib
import sys

sys.path.insert(0, sys.argv[1])  # the checkout (for tests.*)
from tests.test_scan_session import SID, M, agent, session, write  # noqa: E402

out = pathlib.Path(sys.argv[2])
proj, m = session(out)
t0 = 1e9
call = m.launch(t0, "a1", "Lead the refactor")
a1 = agent(proj, "a1", "Lead the refactor", toolUseId=call)
a1.prompt(t0, "brief")
files, parent, pcall = [m, a1], a1, None
for depth, aid in enumerate([f"d{i}" for i in range(2, 11)], start=2):
    pcall = parent.launch(t0 + depth * M, aid, f"level {depth}", model="haiku")
    child = proj.agent(SID, aid, {"description": f"level {depth}", "spawnDepth": depth, "toolUseId": pcall},
                       kind="meta_nested")
    child.prompt(t0 + depth * M, "brief")
    files.append(child)
    parent = child
m.notify(t0 + 6 * M, "a1", call, "completed")
parent.bash(t0 + 59 * M, "still working", pending=True)
write(*files)
doc = {"source": "tests/test_deep.py::test_collector_nests_any_depth_in_one_thread_and_keeps_finished_ancestors, "
                 "its builder run by tools/capture_deep.py",
       "now": t0 + 60 * M, "done_window": 10 * M, "projects": "projects", "events": "events",
       "files": {}, "inbox": [],
       "snapshot": {"tabs": [{"tab_id": "w1:t1", "label": "deep", "workspace_id": "w1"}],
                    "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude",
                                "agent_session": {"kind": "id", "value": SID}}]}}
for p in sorted(out.rglob("*")):
    if p.is_file():
        doc["files"][str(p.relative_to(out))] = {"text": p.read_text(), "mtime": os.path.getmtime(p)}
print(json.dumps(doc, indent=1, ensure_ascii=False))
