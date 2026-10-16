"""Shared fixtures: a fake herdr binary.

The fake records every argv as one JSON line and answers with JSON shaped like
herdr 0.9.3's real ``.result`` payloads (the snapshot comes from the captured
template in fixtures/herdr_snapshot.json via $FAKE_HERDR_SNAPSHOT).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

FAKE_HERDR = r'''#!{python}
import json, os, sys
args = sys.argv[1:]
with open(os.environ["FAKE_HERDR_LOG"], "a") as fh:
    fh.write(json.dumps(args) + "\n")
cmd = " ".join(args[:2])
if cmd in os.environ.get("FAKE_HERDR_FAIL", "").split(","):
    sys.stderr.write(json.dumps({{"error": {{"code": "boom", "message": "fake failure"}}}}))
    sys.exit(1)
if cmd == "api snapshot":
    snap = json.load(open(os.environ["FAKE_HERDR_SNAPSHOT"])) if os.environ.get("FAKE_HERDR_SNAPSHOT") else {{}}
    result = {{"snapshot": snap, "type": "session_snapshot"}}
else:
    result = {{"type": "ok"}}
print(json.dumps({{"id": "cli:" + cmd.replace(" ", ":"), "result": result}}))
'''


class FakeHerdr:
    def __init__(self, tmp_path: Path):
        self.bin = tmp_path / "herdr"
        self.log = tmp_path / "herdr-calls.jsonl"
        self.bin.write_text(FAKE_HERDR.format(python=sys.executable))
        self.bin.chmod(0o755)
        self.log.touch()

    def calls(self) -> list[list[str]]:
        return [json.loads(line) for line in self.log.read_text().splitlines() if line]

    def calls_of(self, *prefix: str) -> list[list[str]]:
        return [c for c in self.calls() if c[: len(prefix)] == list(prefix)]


@pytest.fixture
def fake_herdr(tmp_path, monkeypatch):
    fake = FakeHerdr(tmp_path)
    monkeypatch.setenv("HERDR_BIN_PATH", str(fake.bin))
    monkeypatch.setenv("FAKE_HERDR_LOG", str(fake.log))
    monkeypatch.delenv("HERDR_PLUGIN_CONTEXT_JSON", raising=False)
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    return fake


@pytest.fixture(autouse=True)
def no_real_state(tmp_path, monkeypatch):
    """Tests never read the operator's inbox or event files."""
    monkeypatch.setenv("HERDR_INBOX_DB", str(tmp_path / "no-inbox.db"))
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(tmp_path / "no-events"))
    monkeypatch.setenv("HERDR_BIN_PATH", str(tmp_path / "no-herdr"))
    monkeypatch.setenv("HERDR_PLUGIN_CONFIG_DIR", str(tmp_path / "plugin-config"))
    monkeypatch.delenv("PANORAMA_DONE_WINDOW", raising=False)
    monkeypatch.setenv("PANORAMA_HOOKS_DIR", str(tmp_path / "hooks"))
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "plugin-state"))  # unseen.json, home records
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "xdg-config"))  # herdr's config.toml (theme.py): none
    monkeypatch.delenv("PANORAMA_TRACE", raising=False)
    from herdr_panorama import cockpit  # its process-wide dock state never leaks between tests
    monkeypatch.setattr(cockpit, "moving_until", 0.0)
