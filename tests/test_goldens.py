"""The behaviour goldens (docs/plans/rust-rewrite.md §5.2 items 2 and 3) still describe the code:
tools/capture_goldens.py run again, into a temporary directory, writes the same files as the
committed tests/fixtures/screens/ and tests/fixtures/model/. A failure is a behaviour change.
If it is a deliberate one, regenerate and say why in the PR:

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


def first_difference(want: dict, got: dict, kind: str) -> str:
    if kind == "screens":
        for name in want["screens"]:
            a, b = want["screens"][name], got["screens"].get(name)
            if b is None:
                return f"screen {name!r} is gone"
            for y, (ra, rb) in enumerate(zip(a, b)):
                if ra != rb:
                    return (f"screen {name!r} row {y}:\n  want {''.join(r[0] for r in ra)!r}\n"
                            f"  got  {''.join(r[0] for r in rb)!r}\n  want runs {ra}\n  got runs  {rb}")
        return "screen list or header differs"
    a = json.dumps(want, indent=1, sort_keys=True).splitlines()
    b = json.dumps(got, indent=1, sort_keys=True).splitlines()
    for i, (la, lb) in enumerate(zip(a, b)):
        if la != lb:
            return f"line {i}:\n  want {la}\n  got  {lb}\n  context: {a[max(0, i - 6):i]}"
    return "lengths differ"


@pytest.mark.parametrize("kind", ["model", "screens"])
def test_the_goldens_are_what_the_code_does(tmp_path, kind):
    env = {k: v for k, v in os.environ.items() if not k.startswith("HERDR_")}
    res = subprocess.run([sys.executable, str(ROOT / "tools" / "capture_goldens.py"), "--out", str(tmp_path),
                          "--only", kind], env=env, cwd=ROOT, capture_output=True, text=True, timeout=300)
    assert res.returncode == 0, res.stderr[-3000:]
    want = sorted(p.name for p in (FIX / kind).glob("*.json"))
    got = sorted(p.name for p in (tmp_path / kind).glob("*.json"))
    assert want and got == want
    for name in want:
        a, b = (FIX / kind / name).read_text(), (tmp_path / kind / name).read_text()
        if a != b:
            pytest.fail(f"{kind}/{name}: {first_difference(json.loads(a), json.loads(b), kind)}\n"
                        "a behaviour change; if deliberate: uv run python tools/capture_goldens.py")
