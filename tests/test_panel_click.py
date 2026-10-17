"""Per-tab panels, the operator's report: "clicking any left panel with a mouse results in the
left panel just closing". #44 gave PanelApp a static ``_shown(swarm, visible, focused)`` that
shadowed SwarmApp's ``_shown(pane, tid, aid)``, which every card click calls: the click raised
a TypeError, Textual exited, the core made the panel again. And a viewer that dies of an error
now leaves its traceback next to its crash marker, so the next report has evidence."""

import inspect

import pytest

from herdr_panorama import herdr
from herdr_panorama import panel as viewer
from herdr_panorama.tui import SwarmApp

from .test_panel_sync import panel, paths, state  # noqa: F401 - paths is a fixture

PANELS = {f"w1:t{i}": f"w1:p9{i}" for i in range(1, 10)}


@pytest.mark.parametrize("connected", [True, False])
async def test_a_click_anywhere_on_a_panel_keeps_it_open(paths, monkeypatch, connected):
    snap = {"panes": [{"pane_id": f"w1:p{i}", "tab_id": f"w1:t{i}"} for i in range(1, 10)]
            + [{"pane_id": p, "tab_id": t} for t, p in PANELS.items()]}
    moves = []
    monkeypatch.setattr(herdr, "snapshot", lambda: snap)
    for name in ("focus_tab", "focus_pane", "focus_agent"):
        monkeypatch.setattr(herdr, name, lambda *a, _n=name: moves.append((_n, *a)))
    a = panel(paths, "w1:t2", "w1:p92")
    async with a.run_test(size=(46, 45)) as pilot:
        a.connected = connected
        a._got_state(state(focused="w1:p92", visible=["w1:t2"], panels=PANELS))
        await pilot.pause(0.05)
        targets = list(a.grid.screen_data.targets)
        kinds = {t[3] for t in targets}
        assert {"orch", "new"} <= kinds  # another tab's card, this tab's card, New agent
        for x0, _, y, kind, ident in targets:
            a.arrived.clear()
            a.armed_at = 0.0
            await pilot.click(a.grid, offset=(x0, y))
            for _ in range(20):  # a switch runs on a worker thread
                await pilot.pause(0.02)
                if not a.docking:
                    break
            assert a.is_running and a._exception is None, (kind, ident)
        await pilot.click(a.grid, offset=(1, 44))  # empty space
        await pilot.hover(a.grid, offset=(5, 5))
        await pilot.pause()
        assert a.is_running and a._exception is None
    if connected:
        assert moves  # the clicks did switch
    else:
        assert not moves  # without a core nothing acts


def test_the_panel_overrides_no_method_with_another_signature():
    """The class of the bug: a PanelApp method named like a SwarmApp one must take the same
    arguments, or SwarmApp's own calls to it break."""
    bad = []
    for name, own in vars(viewer.PanelApp).items():
        if name.startswith("__") or not callable(getattr(SwarmApp, name, None)):
            continue
        base = inspect.getattr_static(SwarmApp, name)
        if (isinstance(own, staticmethod) != isinstance(base, staticmethod)
                or list(inspect.signature(getattr(viewer.PanelApp, name)).parameters)
                != list(inspect.signature(getattr(SwarmApp, name)).parameters)):
            bad.append(name)
    assert bad == []


def test_a_crash_leaves_its_traceback_next_to_the_marker(tmp_path, monkeypatch):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))

    class Dying(viewer.PanelApp):
        def __init__(self):
            super().__init__(sock_path=tmp_path / "none.sock", pane_id="w1:p92", tab_id="w1:t2",
                             start_client=False)

        def on_mount(self):
            raise TypeError("a click handler broke")

        def run(self, **kw):
            return super().run(headless=True, size=(46, 45))

    monkeypatch.setattr(viewer, "PanelApp", Dying)
    assert viewer.run() == 1
    assert viewer.crash_marker("w1:p92").exists()
    log = viewer.crash_log("w1:p92").read_text()
    assert "TypeError: a click handler broke" in log and "on_mount" in log


def test_the_crash_log_is_bounded(tmp_path, monkeypatch):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    viewer.mark_crash("w1:p92", RuntimeError("x" * 100_000))
    log = viewer.crash_log("w1:p92").read_text()
    assert len(log) <= viewer.CRASH_LOG_BYTES + 64 and log.rstrip().endswith("x")
