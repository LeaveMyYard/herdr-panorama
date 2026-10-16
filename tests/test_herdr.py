"""The herdr wrapper: sanitised arguments, timeouts, never raises."""

from herdr_panorama import herdr


def test_clean_strips_controls_and_caps():
    assert herdr.clean("a\x1b[31mb\tc\nd\x07") == "a[31mb c d"
    assert len(herdr.clean("x" * 5000)) == 4000


def test_missing_binary_is_an_error_not_an_exception(monkeypatch, tmp_path):
    monkeypatch.setenv("HERDR_BIN_PATH", str(tmp_path / "nope"))
    assert herdr.snapshot() is None
    assert herdr.focus_agent("w1:p1")


def test_snapshot_and_focus_through_the_fake(fake_herdr, monkeypatch, tmp_path):
    from .scenario import SNAP
    import json
    snap = tmp_path / "snap.json"
    snap.write_text(json.dumps(SNAP["result"]["snapshot"]))
    monkeypatch.setenv("FAKE_HERDR_SNAPSHOT", str(snap))
    assert herdr.snapshot()["agents"][0]["agent"] == "claude"
    assert herdr.focus_agent("w1:p1\x1b[2J") is None
    assert fake_herdr.calls()[-1] == ["agent", "focus", "w1:p1[2J"]


def test_open_view_split_targets_the_invoking_pane(fake_herdr, monkeypatch):
    monkeypatch.setenv("HERDR_PLUGIN_CONTEXT_JSON", '{"focused_pane_id": "w1:p9"}')
    monkeypatch.setenv("HERDR_PLUGIN_ROOT", "/plugins/panorama")
    assert herdr.open_view(split=True) is None
    call = fake_herdr.calls()[-1]
    assert call[:7] == ["plugin", "pane", "open", "--plugin", "leavemyyard.herdr-panorama", "--entrypoint", "panorama"]
    assert "--placement" in call and "split" in call and call[call.index("--target-pane") + 1] == "w1:p9"
    assert call[call.index("--cwd") + 1] == "/plugins/panorama"
