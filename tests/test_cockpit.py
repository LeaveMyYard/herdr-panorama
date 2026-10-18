# ruff: noqa: F811 - pytest fixtures arrive as parameters named like the imported fixture
"""Cockpit (experimental): panorama's own pane docks left of the selected agent; agents
never move. Single instance found by content, duplicates closed, undock when off, width,
per-session records; against the stateful fake herdr of test_home (moves like 0.9.3)."""

import itertools
import json
import os

import pytest

from herdr_panorama import cockpit, home
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


def left_docked(h, me, agent):
    """panorama's pane left beside ``agent`` in its tab by an older version's moving cockpit
    (gone since Q187): the pane sits first in the agent's tab, its own home tab is gone."""
    st = state(h)
    old = next(p["tab_id"] for p in st["panes"] if p["pane_id"] == me)
    tab = next(p["tab_id"] for p in st["panes"] if p["pane_id"] == agent)
    pane = next(p for p in st["panes"] if p["pane_id"] == me)
    st["panes"].remove(pane)
    pane["tab_id"] = tab
    st["panes"].insert(next(i for i, p in enumerate(st["panes"]) if p["pane_id"] == agent), pane)
    st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != old]
    put(h, st)


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


def test_the_fake_tab_focus_moves_focus(herdr):
    """The stateful fake herdr's tab focus moves the client there (review of #27), so the
    tests here see a tab focus pull focus, as herdr does."""
    home.ensure(focus=False)
    add_agent(herdr, "billing")
    c, tc = add_agent(herdr, "docs")
    cockpit.herdr.focus_tab(tc)
    assert state(herdr)["focus"] == c


def test_dock_keep_focus_setting_defaults_on():
    configure("cockpit = true\n")
    assert cockpit.keep_focus_setting()
    configure("cockpit = true\ndock_keep_focus = false\n")
    assert not cockpit.keep_focus_setting()


def test_the_key_brings_a_left_docked_panorama_home_and_focuses_it(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    left_docked(herdr, h.pane_id, a)
    assert cli(["open"]) == 0
    assert len(herdr.calls("plugin", "pane", "open")) == 1  # no second panorama
    home_tab = tab_of(herdr, h.pane_id)
    assert home_tab != ta and in_tab(herdr, ta) == [a]
    assert herdr.calls("tab", "focus")[-1] == ["tab", "focus", home_tab]


def test_closing_the_agent_tab_with_panorama_in_it_reopens_a_home(herdr):
    h, _ = home.ensure(focus=False)
    a, ta = add_agent(herdr, "billing")
    left_docked(herdr, h.pane_id, a)
    st = state(herdr)
    st["panes"] = [p for p in st["panes"] if p["tab_id"] != ta]  # the operator closed billing's tab
    st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != ta]
    put(herdr, st)
    h2, _ = home.ensure(focus=False)
    assert h2.pane_id != h.pane_id and len(herdr.calls("plugin", "pane", "open")) == 2
    assert state(herdr)["tabs"][0]["label"] == home.HOME_LABEL


@pytest.mark.parametrize("cockpit_on", [True, False])
def test_a_panorama_left_docked_goes_back_to_its_own_first_tab(herdr, cockpit_on):
    """The moving cockpit is gone (Q187): a home panorama an older version left beside an
    agent goes back to a home tab of its own, the cockpit on (per-tab) or off."""
    h, _ = home.ensure(focus=False)
    add_agent(herdr, "search")
    a, ta = add_agent(herdr, "billing")
    left_docked(herdr, h.pane_id, a)
    configure(f"cockpit = {'true' if cockpit_on else 'false'}\n")
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
    left_docked(herdr, h.pane_id, a)  # tab: [panorama, agent, other] -> panorama first in the root split
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
    left_docked(herdr, h.pane_id, a)
    home.ensure(focus=False)  # panorama goes home; nothing else moves
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


def test_r8f_l4_duplicates_are_left_alone_not_closed(herdr):
    h, _ = home.ensure(focus=False)
    st = state(herdr)
    st["tabs"].append({"tab_id": "w1:t9", "label": "x", "workspace_id": "w1"})
    st["panes"].append({"pane_id": "w1:p9", "tab_id": "w1:t9", "workspace_id": "w1", "label": home.HOME_LABEL,
                        "argv": PANORAMA})
    put(herdr, st)
    assert home.ensure(focus=False)[0] == h
    assert herdr.calls("plugin", "pane", "close") == [] and tab_of(herdr, "w1:p9") == "w1:t9"


def test_r8f_l3_no_foreground_process_is_unknown(herdr):
    put(herdr, {"tabs": [{"tab_id": "w1:t1", "label": "x", "workspace_id": "w1"}],
                "panes": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "argv": []}], "n": 1})
    from herdr_panorama import herdr as herdr_cli
    assert herdr_cli.runs_panorama("w1:p1") is None


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


def test_ensure_with_the_lock_held_elsewhere_gives_up(herdr, lock_held_elsewhere):
    out, took = _timed(lambda: home.ensure(focus=True))
    assert out[0] is None and "holds the home lock" in out[1] and took < 1.0
    assert herdr.calls("plugin", "pane", "open") == []


def test_lock_busy_is_its_own_exception():
    assert issubclass(home.LockBusy, Exception) and not issubclass(home.LockBusy, TimeoutError)
