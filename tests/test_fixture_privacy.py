"""Fixtures and docs carry no real ids (M6): every captured value and key is anonymised."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REAL_ID = re.compile(r"toolu_[A-Za-z0-9]{20,}|\ba[0-9a-f]{16}\b|"
                     r"\b(?!00000000-0000-4000-8000-000000000000)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")


def test_fixtures_hold_no_real_ids():
    for p in (ROOT / "tests" / "fixtures").glob("*.json"):
        hits = REAL_ID.findall(p.read_text())
        assert not hits, f"{p.name}: {hits[:3]}"


def test_docs_hold_no_real_agent_ids():
    for p in [ROOT / "README.md", *(ROOT / "docs").glob("*.md")]:
        assert not re.findall(r"\ba[0-9a-f]{16}\b", p.read_text()), p.name
