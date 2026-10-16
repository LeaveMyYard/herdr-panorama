# ruff: noqa: F811 - pytest fixtures arrive as parameters named like the imported fixture
"""Issue #9: a `◆ panorama` tab must never keep that label once panorama is not in it,
after a quit (`q`) or a crash (the sweep in every ensure)."""

from herdr_panorama import home

from .test_cockpit import beside_home, put, state
from .test_home import herdr  # noqa: F401 - the fixture


def label(h, tab):
    return next((t["label"] for t in state(h)["tabs"] if t["tab_id"] == tab), None)


def drop(h, pane):
    st = state(h)
    st["panes"] = [p for p in st["panes"] if p["pane_id"] != pane]
    put(h, st)


def test_crash_sweep_renames_a_home_tab_left_with_an_agent(herdr):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)  # the operator split it and started Claude
    drop(herdr, h.pane_id)  # panorama crashed: its pane is gone
    h2, _ = home.ensure(focus=False)
    assert label(herdr, tab) == "billing"  # the agent's name, never ◆ panorama
    assert h2.tab_id != tab and label(herdr, h2.tab_id) == home.HOME_LABEL


def test_sweep_falls_back_to_the_kind_then_tab(herdr):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)
    st = state(herdr)
    next(p for p in st["panes"] if p["pane_id"] == other).pop("name")
    put(herdr, st)
    drop(herdr, h.pane_id)
    home.ensure(focus=False)
    assert label(herdr, tab) == "claude"
    h3 = home.ensure(focus=False)[0]
    shell, tab2 = beside_home(herdr, h3.pane_id, agent=False)
    drop(herdr, h3.pane_id)
    home.ensure(focus=False)
    assert label(herdr, tab2) == "tab"


def test_sweep_keeps_a_live_or_unverifiable_panorama_tab(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    beside_home(herdr, h.pane_id)
    home.ensure(focus=False)
    assert label(herdr, h.tab_id) == home.HOME_LABEL  # panorama is still in it
    st = state(herdr)
    next(p for p in st["panes"] if p["pane_id"] == h.pane_id)["argv"] = []  # mid-restore: unknown
    put(herdr, st)
    home.ensure(focus=False)
    assert label(herdr, h.tab_id) == home.HOME_LABEL  # could be panorama: never renamed
    assert not [c for c in herdr.calls("tab", "rename") if c[2] == h.tab_id and c[3] != home.HOME_LABEL]


def test_sweep_ignores_other_tabs(herdr):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    st["tabs"].append({"tab_id": "w1:t8", "label": "work", "workspace_id": "w1"})
    st["panes"].append({"pane_id": "w1:p8", "tab_id": "w1:t8", "workspace_id": "w1", "argv": ["zsh"]})
    put(herdr, st)
    home.ensure(focus=False)
    assert label(herdr, "w1:t8") == "work"


async def test_quit_renames_the_shared_home_tab_first(herdr, monkeypatch):
    from herdr_panorama.tui import SwarmApp
    from .demo import NOW, demo_swarm
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)
    monkeypatch.setenv("HERDR_PANE_ID", h.pane_id)
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True)
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.pause(0.2)
        await pilot.press("q")
        await pilot.pause()
    assert label(herdr, tab) == "billing"


async def test_quit_alone_in_the_home_tab_renames_nothing(herdr, monkeypatch):
    from herdr_panorama.tui import SwarmApp
    from .demo import NOW, demo_swarm
    h, _ = home.ensure(focus=False)
    monkeypatch.setenv("HERDR_PANE_ID", h.pane_id)
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True)
    async with app.run_test(size=(80, 40)) as pilot:
        await pilot.pause(0.2)
        await pilot.press("q")
        await pilot.pause()
    assert label(herdr, h.tab_id) == home.HOME_LABEL  # the tab closes with panorama's pane

