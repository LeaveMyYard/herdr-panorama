"""The behaviour goldens (docs/plans/rust-rewrite.md §5.2 items 2 and 3) still describe the code:
tools/capture_goldens.py run again, into a temporary directory, writes the same files as the
committed tests/fixtures/{inputs,model,screens}/. A failure is a behaviour change. If it is a
deliberate one, regenerate and say why in the PR:

    uv run python tools/capture_goldens.py
"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
FIX = ROOT / "tests" / "fixtures"


def text_of(row) -> str:
    return "".join(r[0] for r in row)


def first_difference(want: dict, got: dict, kind: str) -> str:
    if kind == "screens":
        if want.get("runs") != got.get("runs"):
            return "the runs (inputs or steps) differ"
        for name in want["screens"]:
            a, b = want["screens"][name], got["screens"].get(name)
            if b is None:
                return f"screen {name!r} is gone"
            for y, (ra, rb) in enumerate(zip(a, b)):
                if ra != rb:
                    return (f"screen {name!r} row {y}:\n  want {text_of(ra)!r}\n  got  {text_of(rb)!r}\n"
                            f"  want runs {ra}\n  got runs  {rb}")
            if len(a) != len(b):
                extra = (b if len(b) > len(a) else a)[min(len(a), len(b))]
                return f"screen {name!r}: {len(a)} rows wanted, {len(b)} got; first extra row {text_of(extra)!r}"
        new = [n for n in got["screens"] if n not in want["screens"]]
        return f"new screens {new}" if new else "the header differs"
    a = json.dumps(want, indent=1, sort_keys=True).splitlines()
    b = json.dumps(got, indent=1, sort_keys=True).splitlines()
    for i, (la, lb) in enumerate(zip(a, b)):
        if la != lb:
            return f"line {i}:\n  want {la}\n  got  {lb}\n  context: {a[max(0, i - 6):i]}"
    longer = b if len(b) > len(a) else a
    return f"{len(a)} lines wanted, {len(b)} got; first extra line {longer[min(len(a), len(b))]}"


@pytest.mark.parametrize("kind", ["model", "screens"])
def test_the_goldens_are_what_the_code_does(tmp_path, kind):
    env = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_")}
    res = subprocess.run([sys.executable, str(ROOT / "tools" / "capture_goldens.py"), "--out", str(tmp_path),
                          "--only", kind], env=env, cwd=ROOT, capture_output=True, text=True, timeout=300)
    assert res.returncode == 0, res.stderr[-3000:]
    for sub in ("inputs", kind):
        want = sorted(p.name for p in (FIX / sub).glob("*.json"))
        got = sorted(p.name for p in (tmp_path / sub).glob("*.json"))
        assert want and got == want, (sub, want, got)
        for name in want:
            a, b = (FIX / sub / name).read_text(), (tmp_path / sub / name).read_text()
            if a != b:
                pytest.fail(f"{sub}/{name}: {first_difference(json.loads(a), json.loads(b), sub)}\n"
                            "a behaviour change; if deliberate: uv run python tools/capture_goldens.py")
