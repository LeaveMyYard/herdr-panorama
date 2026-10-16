"""The tailer (port of zoetrope's bytes.rs): partial lines, CRLF, overflow, shrink, replacement."""

import os

from herdr_panorama.scan import tail
from herdr_panorama.scan.tail import LINES, NO_CHANGE, RESET, TailState, consume_bytes, read_appended


def test_partial_line_is_buffered_until_its_newline():
    st = TailState()
    assert consume_bytes(st, b'{"a":1}\n{"b":') == [b'{"a":1}']
    assert consume_bytes(st, b'2}\r\n\n  \n') == [b'{"b":2}']  # CRLF trimmed, blank lines dropped


def test_runaway_line_is_dropped_and_reading_resyncs(monkeypatch):
    monkeypatch.setattr(tail, "MAX_PARTIAL", 10)
    st = TailState()
    assert consume_bytes(st, b"x" * 20) == []
    assert consume_bytes(st, b"yyy\nok\n") == [b"ok"]


def test_read_appended_offsets_shrink_and_replacement(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_bytes(b"one\ntw")
    st = TailState()
    assert read_appended(str(p), st) == (LINES, [b"one"])
    assert read_appended(str(p), st) == (NO_CHANGE, [])
    with open(p, "ab") as fh:
        fh.write(b"o\nthree\n")
    assert read_appended(str(p), st) == (LINES, [b"two", b"three"])
    p.write_bytes(b"new\n")  # shrank: rewritten
    assert read_appended(str(p), st)[0] == RESET
    assert read_appended(str(p), st) == (LINES, [b"new"])
    other = tmp_path / "o.jsonl"
    other.write_bytes(b"new\nlonger file\n")
    os.replace(other, p)  # replaced by a different inode, not shorter
    assert read_appended(str(p), st)[0] == RESET


def test_missing_file(tmp_path):
    assert read_appended(str(tmp_path / "nope"), TailState())[0] == tail.MISSING
