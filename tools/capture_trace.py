#!/usr/bin/env python3
"""Capture Python's PANORAMA_TRACE lines (herdr.trace) for the Rust writer's test:
tests/fixtures/trace/lines.json holds, per case, the raw end time and duration Python rounds,
the op, and the line Python wrote.

    uv run python tools/capture_trace.py
"""

import json
import os
import tempfile
import time
from pathlib import Path

from herdr_panorama import herdr

OUT = Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "trace" / "lines.json"
CASES = [  # (op, wall clock at the end, duration in seconds)
    ("cli api snapshot", 1792000000.1234567, 0.0123456),
    ("socket pane.layout", 1792000000.0005, 0.00005),
    ("socket tab.move", 1792000001.9995, 1.23456789),
    ("cli --machine m1", 1792000002.25, 0.00015),
    ("socket session.snapshot", 1792000003.0, 0.0),
    ("cli plugin config-dir", 1792000004.0625, 12.3449999),
    ("cli --machine caf\u00e9", 1792000005.5, 0.002),  # a machine named in non-ASCII
]


def main() -> None:
    out = []
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "trace.jsonl")
        os.environ["PANORAMA_TRACE"] = path
        real_time, real_perf = time.time, time.perf_counter
        try:
            for op, at, took in CASES:
                started = 1000.0
                time.time = lambda at=at: at
                time.perf_counter = lambda took=took: started + took
                herdr.trace(op, started)
                ms_raw = (started + took - started) * 1000  # what Python rounds
                out.append({"op": op, "at_raw": at, "ms_raw": ms_raw})
        finally:
            time.time, time.perf_counter = real_time, real_perf
        lines = Path(path).read_text().splitlines()
    for case, line in zip(out, lines, strict=True):
        case["line"] = line
    OUT.write_text(json.dumps({"source": "herdr_panorama.herdr.trace (main), run by tools/capture_trace.py",
                               "cases": out}, indent=1) + "\n")


if __name__ == "__main__":
    main()
