#!/usr/bin/env python3
"""Validate the Python-to-Rust test ledger and count its rows by status.

The ledger (tests/ledger/test-ledger.csv, plan docs/plans/rust-rewrite.md §5.2
item 7) maps every collected Python test to its Rust test, or retires it with a
reason. A phase is done when its rows are filled.

Usage:
    python3 tools/ledger_check.py [LEDGER] [--collected FILE]

--collected takes the output of `pytest --collect-only -q` (run with every
HERDR_* variable unset) and also checks that the ledger has exactly one row per
collected test.

--done PHASE (repeatable) checks that a finished phase left nothing behind: no
row of that phase is still todo. --rust-root DIR checks that every Rust test a
mapped row names exists: `path::fn` is a file under DIR holding `fn fn(`. A
`herdr-agent-kit@<tag>:path::fn` reference is checked the same way under
--kit-root when that is given, else skipped; --kit-root must be the kit's
checkout at that tag (the tag in the references is not checked against it).
A reference must name a real test: a `fn` line (not commented out) with a
`#[test]` or `#[tokio::test]` attribute just above it.

Exit status is 0 when the ledger is valid, 1 otherwise.

Standard library only, so it runs without the project's venv.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from collections import Counter
from pathlib import Path

COLUMNS = ["python_test_id", "python_file", "rust_test", "status", "retired_reason", "phase"]
STATUSES = ("mapped", "retired", "todo")
PHASES = {"", "P0a", "P0b", "P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8"}
DEFAULT_LEDGER = Path(__file__).resolve().parent.parent / "tests" / "ledger" / "test-ledger.csv"


def read_collected(path: Path) -> list[str]:
    """Test ids from `pytest --collect-only -q` output (the summary lines are skipped)."""
    return [line.strip() for line in path.read_text().splitlines() if "::" in line]


KIT = re.compile(r"^herdr-agent-kit@[^:]+:")


def missing_rust_tests(refs: str, rust_root: Path | None, kit_root: Path | None) -> list[str]:
    """The references in `refs` (separated by " ; ") whose test function is not found."""
    out = []
    for ref in (r.strip() for r in refs.split(";") if r.strip()):
        root = rust_root
        if KIT.match(ref):
            root, ref = kit_root, KIT.sub("", ref)
        if root is None:
            continue
        path, _, fn = ref.rpartition("::")
        f = root / path
        if not path or not fn or not f.is_file() or not is_test_fn(f.read_text(), fn):
            out.append(ref)
    return out


TEST_ATTR = re.compile(r"^\s*#\[(tokio::)?test\b")


def is_test_fn(text: str, fn: str) -> bool:
    """Whether `text` defines `fn` as a test: a `fn fn(` line that is not commented out, with a
    `#[test]` or `#[tokio::test…]` attribute among the attribute lines just above it."""
    lines = text.splitlines()
    decl = re.compile(rf"^\s*(pub(\([^)]*\))?\s+)?(async\s+)?fn {re.escape(fn)}\(")
    for i, line in enumerate(lines):
        if not decl.match(line):
            continue
        j = i - 1
        while j >= 0 and lines[j].strip().startswith(("#[", "///", "//")):
            if TEST_ATTR.match(lines[j]):
                return True
            j -= 1
    return False


def check(ledger: Path, collected: Path | None, done: tuple[str, ...] = (), rust_root: Path | None = None,
          kit_root: Path | None = None) -> tuple[list[str], Counter[str]]:
    errors: list[str] = []
    counts: Counter[str] = Counter()
    with ledger.open(newline="") as fh:
        reader = csv.reader(fh)
        header = next(reader, None)
        if header != COLUMNS:
            return [f"header is {header!r}, expected {COLUMNS!r}"], counts
        seen: set[str] = set()
        for lineno, row in enumerate(reader, start=2):
            if len(row) != len(COLUMNS):
                errors.append(f"line {lineno}: {len(row)} fields, expected {len(COLUMNS)}")
                continue
            rec = dict(zip(COLUMNS, row))
            tid, status = rec["python_test_id"], rec["status"]
            if not tid:
                errors.append(f"line {lineno}: empty python_test_id")
            elif tid in seen:
                errors.append(f"line {lineno}: duplicate python_test_id {tid}")
            seen.add(tid)
            if rec["python_file"] != tid.split("::", 1)[0]:
                errors.append(f"line {lineno}: python_file {rec['python_file']!r} does not match {tid}")
            if status not in STATUSES:
                errors.append(f"line {lineno}: status {status!r} not one of {STATUSES}")
            elif status == "mapped" and not rec["rust_test"].strip():
                errors.append(f"line {lineno}: mapped without rust_test")
            elif status == "retired" and not rec["retired_reason"].strip():
                errors.append(f"line {lineno}: retired without retired_reason")
            elif status == "todo" and (rec["rust_test"] or rec["retired_reason"]):
                errors.append(f"line {lineno}: todo row has rust_test or retired_reason set")
            if status != "retired" and rec["retired_reason"]:
                errors.append(f"line {lineno}: retired_reason set on a {status!r} row")
            if rec["phase"] not in PHASES:
                errors.append(f"line {lineno}: phase {rec['phase']!r} not one of {sorted(PHASES)}")
            elif status != "todo" and not rec["phase"]:
                errors.append(f"line {lineno}: a {status} row needs the phase that did it")
            if status == "todo" and rec["phase"] in done:
                errors.append(f"line {lineno}: {tid} is still todo in finished phase {rec['phase']}")
            if status == "mapped":
                for ref in missing_rust_tests(rec["rust_test"], rust_root, kit_root):
                    errors.append(f"line {lineno}: no Rust test {ref!r}")
            counts[status] += 1
    if not seen:
        errors.append("the ledger has no rows")
    if collected is not None:
        want = read_collected(collected)
        missing = sorted(set(want) - seen)
        extra = sorted(seen - set(want))
        errors += [f"collected but not in the ledger: {t}" for t in missing]
        errors += [f"in the ledger but not collected: {t}" for t in extra]
    return errors, counts


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("ledger", nargs="?", type=Path, default=DEFAULT_LEDGER)
    ap.add_argument("--collected", type=Path, help="output of `pytest --collect-only -q`")
    ap.add_argument("--done", action="append", default=[], metavar="PHASE",
                    help="a finished phase: none of its rows may be todo")
    ap.add_argument("--rust-root", type=Path, help="this repo's root: check that mapped Rust tests exist")
    ap.add_argument("--kit-root", type=Path, help="herdr-agent-kit's checkout: check the kit's tests too")
    args = ap.parse_args(argv)
    errors, counts = check(args.ledger, args.collected, tuple(args.done), args.rust_root, args.kit_root)
    for err in errors:
        print(f"error: {err}", file=sys.stderr)
    total = sum(counts.values())
    print(f"total {total}: " + ", ".join(f"{s} {counts[s]}" for s in STATUSES))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
