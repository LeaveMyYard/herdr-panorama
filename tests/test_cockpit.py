# ruff: noqa: F811 - pytest fixtures arrive as parameters named like the imported fixture
"""Cockpit (experimental): panorama's own pane docks left of the selected agent; agents
never move. Single instance found by content, duplicates closed, undock when off, width,
per-session records; against the stateful fake herdr of test_home (moves like 0.9.3)."""

import itertools
import json
import os

import pytest

from herdr_panorama import cockpit, home, render
from herdr_panorama.cli import main as cli

from .test_home import herdr  # noqa: F401 - the fixture

_stamp = itertools.count(1_000_000)
PANORAMA = ["uv", "run", "--quiet", "--frozen", "--no-dev", "--project", "/plugins/panorama", "panorama", "tui"]
VENV_PANORAMA = ["/plugins/panorama/.venv/bin/python3", "/plugins/panorama/.venv/bin/panorama", "tui"]


def configure(text: str) -> None:
    d = os.environ["HERDR_PLUGIN_CONFIG_DIR"]
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, "config.toml")
    with open(path, "w") as fh:
        fh.write(text)
    n = next(_stamp)
    os.utime(path, (n, n))  # a fresh mtime: the config cache re-reads


@pytest.fixture(autouse=True)
def on():
    configure("cockpit = true\n")


def state(h):
    return json.loads(h.state.read_text())


def put(h, st):
    h.state.write_text(json.dumps(st))


def add_agent(h, label, extra_panes=0, workspace="w1"):
    """A tab with a Claude agent's pane (and optionally plain panes beside it)."""
    st = state(h)
    st["n"] += 1
    t, p = f"{workspace}:t{st['n']}", f"{workspace}:p{st['n']}"
    st["tabs"].append({"tab_id": t, "label": label, "workspace_id": workspace})
    st["panes"].append({"pane_id": p, "tab_id": t, "workspace_id": workspace, "agent": "claude"})
    for i in range(extra_panes):
        st["panes"].append({"pane_id": f"{p}x{i}", "tab_id": t, "workspace_id": workspace})
    put(h, st)
    return p, t


def tab_of(h, pane):
    return next((p["tab_id"] for p in state(h)["panes"] if p["pane_id"] == pane), None)


def in_tab(h, tab):
    return [p["pane_id"] for p in state(h)["panes"] if p["tab_id"] == tab]


def sockets(h, method):
    return [m["params"] for m in h.moves if m["method"] == method]


def only_panorama_moved(h, me):
    assert {c[2] for c in h.calls("pane", "move")} <= {me}, "an agent's pane was moved"


def test_config_defaults_off_and_width_is_clamped():
    configure("")
    assert not cockpit.enabled() and cockpit.width() == 46
    configure("cockpit = true\ncockpit_width = 500\n")
    assert cockpit.enabled() and cockpit.width() == 119  # stays in the narrow layout
    configure("cockpit = true\ncockpit_width = 20\n")
    assert cockpit.width() == 36  # the header and the footer fit whole from 36 columns
    configure("cockpit = true\nhome_tab = false\n")
    assert not cockpit.enabled()  # the cockpit starts from the home pane


def test_the_home_pane_is_labelled_and_found_by_content(herdr):
    h, _ = home.ensure(focus=False)
    assert herdr.calls("pane", "rename") == [["pane", "rename", h.pane_id, home.HOME_LABEL]]
    home._record().unlink()  # the record is only a hint
    assert home.ensure(focus=False)[0] == h
    assert len(herdr.calls("plugin", "pane", "open")) == 1


def test_dock_moves_only_panorama_left_of_the_agent(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    assert cockpit.dock(a) == (h.pane_id, None)
    assert herdr.calls("pane", "move") == [["pane", "move", h.pane_id, "--tab", ta, "--split", "right", "--no-focus",
                                            "--target-pane", a]]
    # the agent is the swap's source: herdr focuses the source, so focus lands on the agent
    assert herdr.calls("pane", "swap") == [["pane", "swap", "--source-pane", a, "--target-pane", h.pane_id]]
    assert in_tab(herdr, ta) == [h.pane_id, a]  # panorama on the left, the agent where it was
    assert tab_of(herdr, a) == ta and h.tab_id not in [t["tab_id"] for t in state(herdr)["tabs"]]  # empty home closed
    assert sockets(herdr, "layout.set_split_ratio") == [{"tab_id": ta, "path": [], "ratio": (46 + 2) / 200}]
    assert home._load() == {"pane_id": h.pane_id, "tab_id": ta}
    only_panorama_moved(herdr, h.pane_id)
    assert not any(c[1] in ("prompt", "send-keys", "send-text") for c in herdr.calls() if len(c) > 1)


def test_dock_keep_focus_puts_the_operator_on_panorama_and_follow_leaves_it(herdr):
    """dock_keep_focus: panorama is the swap's source, so focus lands on panorama in the agent's
    tab; that focus never starts a follow (the operator is in panorama): no loop."""
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    assert cockpit.dock(a, focus_me=True) == (h.pane_id, None)
    assert herdr.calls("pane", "swap") == [["pane", "swap", "--source-pane", h.pane_id, "--target-pane", a]]
    assert in_tab(herdr, ta) == [h.pane_id, a]  # the same layout as without it: panorama on the left
    assert state(herdr)["focus"] == h.pane_id
    assert sockets(herdr, "pane.focus") == [{"pane_id": h.pane_id}]  # the client follows panorama there
    assert cockpit.follow(h.pane_id) == (None, "the operator is in panorama")
    only_panorama_moved(herdr, h.pane_id)


def test_dock_keep_focus_moves_the_client_to_the_new_tab(herdr):
    """The operator's report (docked next to one agent, Enter on another): panorama left the
    tab, but the client stayed on it, so the panel seemed to close. herdr's swap focuses its
    source only inside its own tab; panorama is focused explicitly, which moves the client."""
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, tb = add_agent(herdr, "search")
    assert cockpit.dock(a)[1] is None
    st = state(herdr)
    st["focus"] = h.pane_id  # the operator is in the docked panel, in a's tab
    put(herdr, st)
    assert cockpit.dock(b, focus_me=True) == (h.pane_id, None)
    assert in_tab(herdr, tb) == [h.pane_id, b] and in_tab(herdr, ta) == [a]
    snap = cockpit.herdr.snapshot()
    assert snap["focused_pane_id"] == h.pane_id and snap["focused_tab_id"] == tb  # on panorama, in b's tab
    # what the open key does, which the attached client follows: the tab, then panorama's pane
    assert herdr.calls("tab", "focus")[-1] == ["tab", "focus", tb]
    assert sockets(herdr, "pane.focus")[-1] == {"pane_id": h.pane_id}
    assert cockpit.follow(h.pane_id) == (None, "the operator is in panorama")


def _docked_next_to_a(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, tb = add_agent(herdr, "search")
    c, tc = add_agent(herdr, "docs")
    assert cockpit.dock(a)[1] is None
    st = state(herdr)
    st["focus"] = h.pane_id  # the operator is in the docked panel, in a's tab
    put(herdr, st)
    return h, (a, ta), (b, tb), (c, tc)


def test_dock_keep_focus_leaves_an_operator_who_moved_on(herdr, monkeypatch):
    """Review of #20: Enter on b, then the operator switches to c while the dock runs: the
    keep-focus calls must not pull them back to b."""
    h, _, (b, tb), (c, tc) = _docked_next_to_a(herdr)
    real = cockpit.herdr.swap_panes

    def swap(source, target):
        err = real(source, target)
        st = state(herdr)
        st["focus"] = c  # prefix+N to c's tab, mid-dock
        put(herdr, st)
        return err
    monkeypatch.setattr(cockpit.herdr, "swap_panes", swap)
    n_tab, n_pane = len(herdr.calls("tab", "focus")), len(sockets(herdr, "pane.focus"))
    assert cockpit.dock(b, focus_me=True) == (h.pane_id, None)
    assert in_tab(herdr, tb) == [h.pane_id, b]  # the dock itself happened
    # only the tab-first focus of b's tab, before the move: nothing after the swap pulls them back
    assert herdr.calls("tab", "focus")[n_tab:] == [["tab", "focus", tb]] and len(sockets(herdr, "pane.focus")) == n_pane
    assert state(herdr)["focus"] == c


def test_an_explicit_dock_shows_the_agents_tab_first(herdr, monkeypatch):
    """Blink measurements (attached client): herdr draws one in-between frame whatever the order;
    focusing the agent's tab before the move makes it the chat asked for, ~20 ms sooner. A
    follow (the operator already there) does not."""
    order = []
    for name in ("focus_tab", "move_pane_to_tab", "swap_panes"):
        real = getattr(cockpit.herdr, name)
        monkeypatch.setattr(cockpit.herdr, name, (lambda r, n: lambda *a, **k: (order.append(n), r(*a, **k))[1])(real, name))
    h, _, (b, tb), _ = _docked_next_to_a(herdr)
    order.clear()
    cockpit.dock(b, focus_me=True)
    assert order[:3] == ["focus_tab", "move_pane_to_tab", "swap_panes"]
    order.clear()
    cockpit.dock(b)  # already next to it: nothing moves, nothing focused
    assert order == []


def test_an_operator_who_moved_on_before_the_dock_is_not_shown_the_agents_tab(herdr, monkeypatch):
    """Review of #27: Enter or an arrow on b, then the operator went to c while the dock waited
    (the home lock): at lock time they are neither on panorama nor in its tab, so no early tab
    focus, and the after-swap check leaves them there too."""
    h, _, (b, tb), (c, tc) = _docked_next_to_a(herdr)
    st = state(herdr)
    st["focus"] = c  # prefix+N to c's tab before the dock took the lock
    put(herdr, st)
    n_tab, n_pane = len(herdr.calls("tab", "focus")), len(sockets(herdr, "pane.focus"))
    assert cockpit.dock(b, focus_me=True) == (h.pane_id, None)
    assert in_tab(herdr, tb) == [h.pane_id, b]  # the dock itself happened
    assert herdr.calls("tab", "focus")[n_tab:] == [] and len(sockets(herdr, "pane.focus")) == n_pane
    assert state(herdr)["focus"] == c


def test_the_fake_tab_focus_moves_focus(herdr):
    """The stateful fake herdr's tab focus moves the client there (review of #27), so the
    tests here see a tab focus pull focus, as herdr does."""
    h, _, (b, tb), (c, tc) = _docked_next_to_a(herdr)
    cockpit.herdr.focus_tab(tc)
    assert state(herdr)["focus"] == c


def test_dock_keep_focus_falls_back_to_the_agent_if_its_tab_cannot_be_focused(herdr, monkeypatch):
    h, _, (b, tb), _ = _docked_next_to_a(herdr)
    monkeypatch.setattr(cockpit.herdr, "focus_tab", lambda tab: "tab_not_found")
    n_pane = len(sockets(herdr, "pane.focus"))
    assert cockpit.dock(b, focus_me=True) == (h.pane_id, None)
    assert herdr.calls("agent", "focus")[-1] == ["agent", "focus", b]  # at least the agent, as without keep
    assert len(sockets(herdr, "pane.focus")) == n_pane


def test_dock_keep_focus_traces_each_focus_step(herdr, monkeypatch, tmp_path):
    trace = tmp_path / "trace.jsonl"
    monkeypatch.setenv("PANORAMA_TRACE", str(trace))
    h, _, (b, tb), _ = _docked_next_to_a(herdr)
    cockpit.dock(b, focus_me=True)
    steps = [json.loads(x) for x in trace.read_text().splitlines() if '"keep-focus' in x]
    assert [s["op"] for s in steps] == ["keep-focus after swap", "keep-focus after tab focus",
                                        "keep-focus after pane focus"]
    assert steps[-1]["focused_tab"] == tb and steps[-1]["focused_pane"] == h.pane_id


def test_dock_keep_focus_without_a_swap_still_focuses_panorama(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    monkeypatch.setattr(cockpit.herdr, "swap_panes", lambda s, t: "not swapped: test")
    assert cockpit.dock(a, focus_me=True) == (h.pane_id, None)
    assert sockets(herdr, "pane.focus") == [{"pane_id": h.pane_id}]


def test_dock_keep_focus_setting_defaults_on():
    configure("cockpit = true\n")
    assert cockpit.keep_focus_setting()
    configure("cockpit = true\ndock_keep_focus = false\n")
    assert not cockpit.keep_focus_setting()


def test_docking_elsewhere_leaves_the_previous_agent_alone_in_its_tab(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    b, tb = add_agent(herdr, "search", extra_panes=1)
    cockpit.dock(a)
    cockpit.dock(b)
    assert in_tab(herdr, ta) == [a]
    assert in_tab(herdr, tb)[0] == h.pane_id and set(in_tab(herdr, tb)) == {h.pane_id, b, b + "x0"}
    only_panorama_moved(herdr, h.pane_id)
    assert cockpit.dock(b) == (h.pane_id, None) and len(herdr.calls("pane", "move")) == 2  # already there


def test_a_failed_move_leaves_panorama_where_it_was(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    monkeypatch.setenv("FAKE_HERDR_FAIL", "pane move --tab")
    done, err = cockpit.dock(a)
    assert done is None and "could not move panorama" in err
    assert tab_of(herdr, h.pane_id) == h.tab_id and in_tab(herdr, ta) == [a]


def test_non_agents_are_not_docked_to(herdr):
    h, _ = home.ensure(focus=False)
    assert cockpit.dock("w1:p99")[0] is None
    assert cockpit.dock(h.pane_id)[0] is None  # panorama itself runs no agent
    assert herdr.calls("pane", "move") == []


def test_the_key_finds_a_docked_panorama_and_focuses_the_list(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    cockpit.dock(a)
    assert cli(["open"]) == 0
    assert len(herdr.calls("plugin", "pane", "open")) == 1  # no second panorama
    assert herdr.calls("tab", "focus")[-1] == ["tab", "focus", ta]
    assert sockets(herdr, "pane.focus") == [{"pane_id": h.pane_id}]


def test_closing_the_agent_tab_with_panorama_in_it_reopens_a_home(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    cockpit.dock(a)
    st = state(herdr)
    st["panes"] = [p for p in st["panes"] if p["tab_id"] != ta]  # the operator closed billing's tab
    st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != ta]
    put(herdr, st)
    h2, _ = home.ensure(focus=False)
    assert h2.pane_id != h.pane_id and len(herdr.calls("plugin", "pane", "open")) == 2
    assert state(herdr)["tabs"][0]["label"] == home.HOME_LABEL


def test_cockpit_off_brings_panorama_back_to_its_own_first_tab(herdr):
    h, _ = home.ensure(focus=False)
    add_agent(herdr, "search")
    a, ta = add_agent(herdr, "billing")
    cockpit.dock(a)
    configure("cockpit = false\n")
    h2, _ = home.ensure(focus=False)
    assert herdr.calls("pane", "move")[-1] == ["pane", "move", h.pane_id, "--new-tab", "--label", home.HOME_LABEL,
                                               "--no-focus", "--workspace", "w1"]
    assert state(herdr)["tabs"][0]["label"] == home.HOME_LABEL and in_tab(herdr, h2.tab_id) == [h.pane_id]
    assert in_tab(herdr, ta) == [a]
    only_panorama_moved(herdr, h.pane_id)


def test_an_older_home_pane_is_adopted_and_labelled(herdr):
    st = {"tabs": [{"tab_id": "w1:t1", "label": "❓2 " + home.HOME_LABEL, "workspace_id": "w1"}],
          "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "label": "Panorama",
                     "argv": VENV_PANORAMA}], "n": 1}
    put(herdr, st)
    assert home.ensure(focus=False)[0].pane_id == "w1:p1"
    assert herdr.calls("plugin", "pane", "open") == []
    assert ["pane", "rename", "w1:p1", home.HOME_LABEL] in herdr.calls("pane", "rename")


def test_records_are_per_herdr_session(herdr, monkeypatch):
    first = home._record()
    monkeypatch.setenv("HERDR_SOCKET_PATH", "/elsewhere/other-session.sock")
    assert home._record() != first  # another `herdr --session` never reads or overwrites ours


def test_fit_sizes_panoramas_split_and_leaves_a_dragged_one(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing", extra_panes=1)
    cockpit.dock(a)  # tab: [panorama, agent, other] -> panorama first in the root split
    total = cockpit.fit(h.pane_id, ta)
    assert total == 198 and sockets(herdr, "layout.set_split_ratio")[-1] == {"tab_id": ta, "path": [],
                                                                             "ratio": (46 + 2) / 198}
    n = len(sockets(herdr, "layout.set_split_ratio"))
    assert cockpit.fit(h.pane_id, ta, total) == total and len(sockets(herdr, "layout.set_split_ratio")) == n
    herdr.area = 300
    cockpit.fit(h.pane_id, ta, total)
    assert sockets(herdr, "layout.set_split_ratio")[-1]["ratio"] == (46 + 2) / 300


def test_split_path_and_side():
    root = {"type": "split", "direction": "down", "first": {"type": "pane", "pane_id": "a"},
            "second": {"type": "split", "direction": "right", "first": {"type": "pane", "pane_id": "b"},
                       "second": {"type": "pane", "pane_id": "me"}}}
    path, node, first = cockpit._split_of(root, "me", [])
    assert path == [True] and not first and cockpit._pane_ids(node) == ["b", "me"]
    assert cockpit._split_of({"type": "pane", "pane_id": "me"}, "me", []) is None


# -- the view ----------------------------------------------------------------------------

async def _app(fake_herdr):
    from herdr_panorama.tui import SwarmApp
    from .demo import NOW, demo_swarm
    return SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1",
                    home_mode=True, cockpit_mode=True)


async def _loaded(app, pilot):
    for _ in range(50):
        await pilot.pause(0.02)
        if app.loaded:
            break
    app.armed_at = 0.0


async def test_enter_docks_and_focuses_the_agent(fake_herdr, monkeypatch):
    docked = []
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (docked.append(p), (me, None))[1])
    app = await _app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        await pilot.press("down", "enter")
        for _ in range(40):
            await pilot.pause(0.05)
            if fake_herdr.calls_of("agent", "focus"):
                break
        assert docked == ["w1:p2"] and app.is_running
        await pilot.press("q")
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p2"]]


def _record_docks(monkeypatch) -> list:
    docked: list = []

    def dock(p, me=None, on_docked=None, focus_me=False):
        docked.append((p, focus_me))
        return me or "w1:p9", None
    monkeypatch.setattr(cockpit, "dock", dock)
    return docked


async def _docked_app(fake_herdr):
    from herdr_panorama.tui import SwarmApp
    from .design_dock import design_swarm
    return SwarmApp(source=design_swarm, live_refresh=False, clock=lambda: 0.0, focus_pane="w1:p3",
                    home_mode=True, cockpit_mode=True)  # docked next to tab 3: the 3a panel


async def _settled(app, pilot):
    for _ in range(40):
        await pilot.pause(0.05)
        if not app.docking:
            return


@pytest.mark.parametrize("key,keep,arrows", [("enter", False, True), ("g", False, True), ("enter", True, False)])
async def test_docked_enter_browses_and_g_goes_into_the_agent(fake_herdr, monkeypatch, key, keep, arrows):
    """In the docked panel, with ↑/↓ switching the chats (dock_on_arrows, the default), Enter goes
    into the agent like g. With dock_on_arrows off, Enter docks and keeps the keyboard (#14)."""
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: arrows)
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        assert app.view.dock
        target = next(o.pane_id for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)
        app._select(0, target)
        await pilot.press(key)
        await _settled(app, pilot)
        assert docked == [(target, keep)]
        await pilot.press("q")
    assert fake_herdr.calls_of("agent", "focus") == ([] if keep else [["agent", "focus", target]])


async def _arrow(app, pilot, *keys, wait=0.5):
    await pilot.press(*keys)
    for _ in range(int(wait / 0.05)):
        await pilot.pause(0.05)
    await _settled(app, pilot)


async def test_arrows_switch_the_chats_once_they_settle(fake_herdr, monkeypatch):
    """The operator's proposal: in the docked panel ↓ docks next to the highlighted orchestrator
    after the arrows rest, the keyboard staying on panorama; a quick run of arrows docks once,
    at the last one."""
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        await _arrow(app, pilot, "down")
        first = app.view.orch
        assert docked == [(first, True)]
        await _arrow(app, pilot, "up", "up", "up", "down")  # held / quick: one dock, at the last
        assert len(docked) == 2 and docked[-1] == (app.view.orch, True)
        await pilot.press("q")
    assert fake_herdr.calls_of("agent", "focus") == []  # the keyboard never left panorama


async def test_arrows_do_not_dock_the_docked_one_new_agent_or_a_remote(fake_herdr, monkeypatch):
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        app._select(0, app.swarm.docked)
        app.action_move(0)  # an arrow that stays on the docked entry
        await _arrow(app, pilot)
        assert docked == []
        app._select(0, render.NEW_ROW)
        app.action_move(0)
        await _arrow(app, pilot)
        assert docked == []
        remote = next(o for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)
        remote.machine = "build-box"  # an orchestrator on another machine: Enter's hint, never a dock
        app._select(0, remote.pane_id)
        app.action_move(0)
        await _arrow(app, pilot)
        assert docked == []
        await pilot.press("q")


async def test_arrows_off_or_the_2a_graph_never_dock_on_arrows(fake_herdr, monkeypatch):
    docked = _record_docks(monkeypatch)
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: False)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        await _arrow(app, pilot, "down")
        assert docked == []
        await pilot.press("q")
    monkeypatch.setattr(cockpit, "arrows_setting", lambda: True)
    wide = await _docked_app(fake_herdr)
    async with wide.run_test(size=(160, 45)) as pilot:
        await _loaded(wide, pilot)
        assert not wide.view.dock
        await _arrow(wide, pilot, "down")
        assert docked == []
        await pilot.press("q")


@pytest.mark.parametrize("why", ["following", "blurred", "herdr says another pane"])
async def test_an_arrow_dock_is_skipped_when_the_keyboard_left_or_a_follow_runs(fake_herdr, monkeypatch, why):
    """Review of #26: when the arrows' timer fires, panorama must still have the keyboard and no
    follow may be moving it."""
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        app.my_pane = "w1:p9"
        await pilot.press("down")
        if why == "following":
            app.following = True
        elif why == "blurred":
            app.pane_focused = False
        else:
            app.herdr_focused_pane = "w1:p1"  # herdr's last report: the operator is in another pane
        await _arrow(app, pilot, wait=0.4)
        assert docked == []
        await pilot.press("q")


async def test_an_arrow_settling_during_a_dock_runs_after_it_even_if_focus_looks_elsewhere(fake_herdr, monkeypatch):
    """Review of #26 (Low): a dock across workspaces briefly makes the focus checks wrong (a new
    pane id); "a dock is running" is checked first, so the arrow runs after the dock, not lost."""
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        app.docking = True
        app.my_pane, app.herdr_focused_pane = "w5:p1", "w7:p2"  # mid-dock: the new id not known yet
        await _arrow(app, pilot, "down", wait=0.4)
        assert docked == [] and app._arrow_again
        app.my_pane = "w7:p2"  # the dock returned it
        app._docked(None)
        await _arrow(app, pilot, wait=0.4)
        assert docked == [(app.view.orch, True)]
        await pilot.press("q")


async def test_an_arrow_dock_due_during_a_dock_runs_after_it(fake_herdr, monkeypatch):
    docked = _record_docks(monkeypatch)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        app.docking = True  # a dock is running
        await _arrow(app, pilot, "down", wait=0.4)
        assert docked == [] and app._arrow_again
        app._docked(None)  # it finished: the settled arrow docks now
        await _arrow(app, pilot, wait=0.4)
        assert docked == [(app.view.orch, True)]
        await pilot.press("q")


async def test_docked_enter_with_keep_focus_off_focuses_the_agent(fake_herdr, monkeypatch):
    docked = _record_docks(monkeypatch)
    monkeypatch.setattr(cockpit, "keep_focus_setting", lambda: False)
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        target = next(o.pane_id for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)
        app._select(0, target)
        await pilot.press("enter")
        await _settled(app, pilot)
        await pilot.press("q")
    assert docked == [(target, False)] and fake_herdr.calls_of("agent", "focus") == [["agent", "focus", target]]


async def test_a_failed_keep_focus_dock_still_goes_to_the_agent(fake_herdr, monkeypatch):
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None, focus_me=False: (None, "no move"))
    app = await _docked_app(fake_herdr)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        target = next(o.pane_id for o in app.swarm.orchestrators if o.pane_id != app.swarm.docked)
        app._select(0, target)
        await pilot.press("enter")
        await _settled(app, pilot)
        await pilot.press("q")
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", target]]


async def test_the_2a_graph_enter_still_focuses_the_agent(fake_herdr, monkeypatch):
    """The full graph (wide, not the docked panel) is unchanged: Enter goes into the agent."""
    docked = _record_docks(monkeypatch)
    app = await _app(fake_herdr)
    async with app.run_test(size=(160, 45)) as pilot:
        await _loaded(app, pilot)
        assert not app.view.dock
        await pilot.press("down", "enter")
        await _settled(app, pilot)
        await pilot.press("q")
    assert docked and all(not keep for _, keep in docked)
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", docked[0][0]]]


async def test_a_failed_dock_still_goes_to_the_agent(fake_herdr, monkeypatch):
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (None, "could not move panorama"))
    app = await _app(fake_herdr)
    notes = []
    monkeypatch.setattr(app, "notify", lambda msg, **kw: notes.append(msg))
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        await pilot.press("g")
        for _ in range(40):
            await pilot.pause(0.05)
            if notes:
                break
    assert fake_herdr.calls_of("agent", "focus") == [["agent", "focus", "w1:p1"]]
    assert notes and "could not move panorama" in notes[0]


# -- look-alikes (review of #8) ------------------------------------------------------------

def lookalike(h, pane_label=None, tab_label="x", argv=None, agent=None):
    st = state(h)
    st["n"] += 1
    t, p = f"w1:t{st['n']}", f"w1:p{st['n']}"
    st["tabs"].insert(0, {"tab_id": t, "label": tab_label, "workspace_id": "w1"})  # first: wins on order
    pane = {"pane_id": p, "tab_id": t, "workspace_id": "w1", "argv": argv or ["zsh"]}
    if pane_label:
        pane["label"] = pane_label
    if agent:
        pane["agent"] = agent
    st["panes"].insert(0, pane)
    put(h, st)
    return p


@pytest.mark.parametrize("kind", ["shell with the label", "panorama exited, its shell left",
                                  "another plugin alone in a ◆ tab", "label restored without its process",
                                  "an agent that just exited"])
def test_r8_lookalikes_are_never_adopted_renamed_moved_or_closed(herdr, kind):
    h, _ = home.ensure(focus=False)
    spec = {"shell with the label": dict(pane_label=home.HOME_LABEL),
            "panorama exited, its shell left": dict(pane_label=home.HOME_LABEL, tab_label=home.HOME_LABEL),
            "another plugin alone in a ◆ tab": dict(tab_label=home.HOME_LABEL,
                                                    argv=["uv", "run", "--project", "/plugins/other", "other", "tui"]),
            "label restored without its process": dict(pane_label=home.HOME_LABEL, argv=["sleep", "900"]),
            "an agent that just exited": dict(tab_label=home.HOME_LABEL, argv=["zsh"])}[kind]
    fake = lookalike(herdr, **spec)
    home._record().unlink()  # no tie-breaker: order alone would pick the look-alike
    assert home.ensure(focus=False)[0] == h
    a, _ = add_agent(herdr, "billing")
    cockpit.dock(a)
    calls = herdr.calls()
    assert not any(fake in c for c in calls if c[:2] in (["pane", "rename"], ["pane", "move"], ["pane", "swap"])
                   or c[:3] == ["plugin", "pane", "close"])
    assert tab_of(herdr, fake) is not None  # untouched
    only_panorama_moved(herdr, h.pane_id)


def test_r8_only_a_lookalike_left_means_a_new_panorama_not_an_adoption(herdr):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    for p in st["panes"]:
        if p["pane_id"] == h.pane_id:
            p["argv"] = ["zsh"]  # panorama quit; its shell stays with the label
    put(herdr, st)
    h2, _ = home.ensure(focus=False)
    assert h2.pane_id != h.pane_id and len(herdr.calls("plugin", "pane", "open")) == 2
    assert herdr.calls("plugin", "pane", "close") == []


def test_r8_unverifiable_lookalikes_open_nothing(herdr, monkeypatch):
    home.ensure(focus=False)
    monkeypatch.setenv("FAKE_HERDR_FAIL", "pane process-info")
    h, err = home.ensure(focus=False)
    assert h is None and "verify" in err
    assert len(herdr.calls("plugin", "pane", "open")) == 1 and herdr.calls("plugin", "pane", "close") == []
    assert cli(["open"]) == 0  # the key still works: the popup
    assert "--placement" not in herdr.calls("plugin", "pane", "open")[-1]


@pytest.mark.parametrize("argv,runs", [(PANORAMA, True), (VENV_PANORAMA, True), (["panorama", "tui"], True),
                                       (["zsh"], False), (["panorama", "snapshot"], False),
                                       (["uv", "run", "other", "tui"], False), (["vim", "panorama.md"], False)])
def test_r8_process_match(herdr, argv, runs):
    st = {"tabs": [{"tab_id": "w1:t1", "label": "x", "workspace_id": "w1"}],
          "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "argv": argv}], "n": 1}
    put(herdr, st)
    from herdr_panorama import herdr as herdr_cli
    assert herdr_cli.runs_panorama("w1:p1") is runs


# -- final review of #8 --------------------------------------------------------------------

def beside_home(h, home_pane, agent=True):
    """A pane the operator split off in the home tab (Claude in it, or a shell)."""
    st = state(h)
    tab = next(p["tab_id"] for p in st["panes"] if p["pane_id"] == home_pane)
    pane = {"pane_id": "w1:p50", "tab_id": tab, "workspace_id": "w1"}
    if agent:
        pane.update(agent="claude", name="billing")
    st["panes"].append(pane)
    put(h, st)
    return "w1:p50", tab


@pytest.mark.parametrize("cockpit_on", [False, True])
def test_r8f_h1_a_shared_home_tab_is_left_alone_with_the_cockpit_off(herdr, cockpit_on):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)
    configure(f"cockpit = {'true' if cockpit_on else 'false'}\n")
    assert home.ensure(focus=False)[0] == h
    assert herdr.calls("pane", "move") == [] and herdr.calls("plugin", "pane", "close") == []
    assert tab_of(herdr, h.pane_id) == tab


def test_r8f_h1_docking_out_of_a_shared_home_tab_relabels_it_first(herdr):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)
    a, ta = add_agent(herdr, "search")
    assert cockpit.dock(a, h.pane_id) == (h.pane_id, None)
    label = next(t["label"] for t in state(herdr)["tabs"] if t["tab_id"] == tab)
    assert label == "billing" and not home.is_home_label(label)  # never an agent under panorama's label
    renames = [c for c in herdr.calls() if c[:2] == ["tab", "rename"]]
    moves = [c for c in herdr.calls() if c[:2] == ["pane", "move"]]
    assert herdr.calls().index(renames[-1]) < herdr.calls().index(moves[0])  # relabelled before leaving


def test_r8f_h1_an_unnamed_agent_left_behind_gives_its_kind(herdr):
    h, _ = home.ensure(focus=False)
    other, tab = beside_home(herdr, h.pane_id)
    st = state(herdr)
    next(p for p in st["panes"] if p["pane_id"] == other).pop("name")
    put(herdr, st)
    a, _ = add_agent(herdr, "search")
    cockpit.dock(a, h.pane_id)
    assert next(t["label"] for t in state(herdr)["tabs"] if t["tab_id"] == tab) == "claude"


def test_r8f_h1_a_failed_relabel_refuses_to_dock(herdr, monkeypatch):
    h, _ = home.ensure(focus=False)
    beside_home(herdr, h.pane_id, agent=False)
    a, _ = add_agent(herdr, "search")
    monkeypatch.setenv("FAKE_HERDR_FAIL", "tab rename")
    done, err = cockpit.dock(a, h.pane_id)
    assert done is None and "panorama" in err and herdr.calls("pane", "move") == []


def test_r8f_l4_duplicates_are_left_alone_not_closed(herdr):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    st["tabs"].append({"tab_id": "w1:t9", "label": "x", "workspace_id": "w1"})
    st["panes"].append({"pane_id": "w1:p9", "tab_id": "w1:t9", "workspace_id": "w1", "label": home.HOME_LABEL,
                        "argv": PANORAMA})
    put(herdr, st)
    assert home.ensure(focus=False)[0] == h
    assert herdr.calls("plugin", "pane", "close") == [] and tab_of(herdr, "w1:p9") == "w1:t9"


def test_r8f_l2_dock_moves_the_instance_whose_key_was_pressed(herdr):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    st["tabs"].append({"tab_id": "w1:t9", "label": "x", "workspace_id": "w1"})
    st["panes"].append({"pane_id": "w1:p9", "tab_id": "w1:t9", "workspace_id": "w1", "label": home.HOME_LABEL,
                        "argv": PANORAMA})
    put(herdr, st)
    a, _ = add_agent(herdr, "billing")
    assert cockpit.dock(a, "w1:p9")[0] == "w1:p9"
    assert herdr.calls("pane", "move")[0][2] == "w1:p9" and tab_of(herdr, h.pane_id) == h.tab_id


def test_r8f_l3_no_foreground_process_is_unknown(herdr):
    put(herdr, {"tabs": [{"tab_id": "w1:t1", "label": "x", "workspace_id": "w1"}],
                "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "argv": []}], "n": 1})
    from herdr_panorama import herdr as herdr_cli
    assert herdr_cli.runs_panorama("w1:p1") is None


def test_r8f_nit_dock_follows_a_new_id_across_workspaces(herdr):
    h, _ = home.ensure(focus=False)
    far, _ = add_agent(herdr, "elsewhere", workspace="w2")
    mine, err = cockpit.dock(far, h.pane_id)
    assert err is None and mine.startswith("w2:") and mine != h.pane_id
    assert home._load()["pane_id"] == mine and tab_of(herdr, mine) == tab_of(herdr, far)
    assert herdr.calls("pane", "swap")[-1][5] == mine  # the swap uses the new id


def test_r8f_nit_dock_holds_the_home_lock(herdr, monkeypatch):
    import fcntl
    h, _ = home.ensure(focus=False)
    a, _ = add_agent(herdr, "billing")
    seen = []
    real = cockpit.herdr.move_pane_to_tab

    def probe(*args, **kw):
        fd = os.open(home.state_dir() / f"home-{home.session_key()}.lock", os.O_WRONLY)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            seen.append("free")
            fcntl.flock(fd, fcntl.LOCK_UN)
        except BlockingIOError:
            seen.append("held")
        finally:
            os.close(fd)
        return real(*args, **kw)
    monkeypatch.setattr(cockpit.herdr, "move_pane_to_tab", probe)
    cockpit.dock(a, h.pane_id)
    assert seen == ["held"]


async def test_r8f_m1_l1_fit_waits_for_the_dock_and_uses_a_verified_pane(fake_herdr, monkeypatch):
    fits, verified = [], {"home": home.Home("w1:p1", "w1:t1")}
    monkeypatch.setattr(cockpit, "fit", lambda pane, tab, last=None: fits.append((pane, tab, last)) or 100)
    monkeypatch.setattr(home, "find", lambda snap, found=None, prefer=None: verified["home"])
    monkeypatch.setattr(cockpit.herdr, "snapshot", lambda: {"panes": [], "tabs": []})
    app = await _app(fake_herdr)
    home._save(home.Home("w1:p7", "w1:t7"))  # a stale record: never used for fitting
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        app.docking = True
        app._fit()
        assert fits == []  # never while a dock is under way
        app.docking = False
        app._fit()
        assert fits == [("w1:p1", "w1:t1", None)]  # the verified pane, not the record
        verified["home"] = None
        app._fit()
        assert len(fits) == 1  # no verified panorama pane: nothing is resized


async def test_r8f_l5_enter_reads_the_cockpit_setting_live(fake_herdr, monkeypatch):
    docked = []
    monkeypatch.setattr(cockpit, "dock", lambda p, me=None, on_docked=None: (docked.append(p), (me, None))[1])
    from herdr_panorama.tui import SwarmApp
    from .demo import NOW, demo_swarm
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=True)
    async with app.run_test(size=(46, 45)) as pilot:
        await _loaded(app, pilot)
        configure("cockpit = false\n")
        await pilot.press("g")
        await pilot.pause(0.3)
        assert docked == []  # turned off while running: g just goes to the pane
        configure("cockpit = true\n")
        app.armed_at = 0.0
        await pilot.press("g")
        for _ in range(40):
            await pilot.pause(0.05)
            if docked:
                break
        assert docked == ["w1:p1"]


# -- the home lock held elsewhere (review of #16) --------------------------------------------

@pytest.fixture
def lock_held_elsewhere(monkeypatch):
    """Another process holds the home lock; every wait for it is cut to 0.2 s."""
    import threading
    held, release = threading.Event(), threading.Event()

    def hold():
        with home._lock():
            held.set()
            release.wait(10)
    threading.Thread(target=hold, daemon=True).start()
    held.wait(2)
    monkeypatch.setattr(home, "LOCK_SECONDS", 0.2)
    yield
    release.set()


def _timed(fn):
    import time as _t
    start = _t.monotonic()
    out = fn()
    return out, _t.monotonic() - start


def test_dock_with_the_lock_held_elsewhere_gives_up(herdr, lock_held_elsewhere):
    out, took = _timed(lambda: cockpit.dock("w1:p1", "w1:p9"))
    assert out == (None, "another panorama operation holds the home lock") and took < 1.0
    assert herdr.calls("pane", "move") == []


def test_follow_with_the_lock_held_elsewhere_gives_up(herdr, lock_held_elsewhere):
    out, took = _timed(lambda: cockpit.follow("w1:p9", None))
    assert out == (None, "another panorama operation holds the home lock") and took < 1.0


def test_ensure_with_the_lock_held_elsewhere_gives_up(herdr, lock_held_elsewhere):
    out, took = _timed(lambda: home.ensure(focus=True))
    assert out[0] is None and "holds the home lock" in out[1] and took < 1.0
    assert herdr.calls("plugin", "pane", "open") == []


def test_lock_busy_is_its_own_exception():
    assert issubclass(home.LockBusy, Exception) and not issubclass(home.LockBusy, TimeoutError)
