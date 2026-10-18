"""tools/ledger_check.py: the per-phase completeness check and the Rust test references."""

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("ledger_check", ROOT / "tools" / "ledger_check.py")
lc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lc)

HEADER = "python_test_id,python_file,rust_test,status,retired_reason,phase\n"


def ledger(tmp_path, *rows):
    p = tmp_path / "ledger.csv"
    p.write_text(HEADER + "".join(r + "\n" for r in rows))
    return p


def test_a_finished_phase_may_leave_no_todo(tmp_path):
    p = ledger(tmp_path, "tests/a.py::t1,tests/a.py,,todo,,P3", "tests/a.py::t2,tests/a.py,,todo,,P4")
    errors, _ = lc.check(p, None, done=("P3",))
    assert len(errors) == 1 and "t1" in errors[0] and "finished phase P3" in errors[0]
    assert lc.check(p, None, done=("P5",))[0] == []


def test_a_mapped_or_retired_row_names_its_phase(tmp_path):
    p = ledger(tmp_path, "tests/a.py::t1,tests/a.py,src/x.rs::f,mapped,,")
    assert any("needs the phase" in e for e in lc.check(p, None)[0])


def test_mapped_rows_name_rust_tests_that_exist(tmp_path):
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "x.rs").write_text("#[test]\nfn there() {}\nfn there_too_long() {}\n")
    kit = "herdr-agent-kit@v0.1.0:tests/k.rs::kit_fn"
    p = ledger(tmp_path, f"tests/a.py::t1,tests/a.py,src/x.rs::there ; {kit},mapped,,P3",
               "tests/a.py::t2,tests/a.py,src/x.rs::there_to,mapped,,P3",
               "tests/a.py::t3,tests/a.py,src/gone.rs::there,mapped,,P3")
    errors = lc.check(p, None, rust_root=tmp_path)[0]  # the kit's refs: skipped without --kit-root
    assert [e.split(": ", 1)[1] for e in errors] == ["no Rust test 'src/x.rs::there_to'",
                                                     "no Rust test 'src/gone.rs::there'"]
    (tmp_path / "tests").mkdir()
    errors = lc.check(p, None, rust_root=tmp_path, kit_root=tmp_path)[0]
    assert "no Rust test 'tests/k.rs::kit_fn'" in "\n".join(errors)
