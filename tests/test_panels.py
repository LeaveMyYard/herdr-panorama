"""Per-tab panels, part 2: the panel manager (panels.py) against an in-memory herdr: the start
batch and its focus restore, the operator's tab only afterwards, ownership, closed stays
closed, duplicates, orphans, the open key, the viewers' switching and + New agent."""

from types import SimpleNamespace

import pytest

from herdr_panorama import cockpit, core, herdr, home, panels, protocol


class World:
    """Tabs of panes (left to right), the focus, and herdr's focus events, as the manager sees
    them through the herdr module."""

    def __init__(self, monkeypatch, tabs: dict[str, list[dict]], focus: tuple[str, str]):
        self.tabs = tabs
        self.focus = focus
        self.calls: list[tuple] = []
        self.log = cockpit.FocusLog()
        self.log.live = True
        self.n = 100
        self.argv: dict[str, str] = {}  # pane -> "panel" for panes that run our viewer
        self.on_swap = None
        mp = monkeypatch
        mp.setattr(herdr, "snapshot", self.snapshot)
        mp.setattr(herdr, "open_panel", self.open_panel)
        mp.setattr(herdr, "swap_panes", self.swap)
        mp.setattr(herdr, "close_plugin_pane", self.close)
        mp.setattr(herdr, "focus_tab", self.focus_tab)
        mp.setattr(herdr, "focus_pane", self.focus_pane)
        mp.setattr(herdr, "focus_agent", self.focus_pane)
        mp.setattr(herdr, "runs_panorama", lambda pane, cmd="tui", pid=None: self.argv.get(pane) == cmd)
        mp.setattr(cockpit, "fit", lambda pane, tab, last=None: None)
        mp.setattr(home, "find", lambda snap, found=None, prefer=None: home.Home("w1:p1", "w1:t1"))
        mp.setattr(cockpit, "follow_tabs", lambda: "any")

    def pane(self, pid):
        return next(p for ps in self.tabs.values() for p in ps if p["pane_id"] == pid)

    def tab_of(self, pid):
        return next(t for t, ps in self.tabs.items() if any(p["pane_id"] == pid for p in ps))

    def snapshot(self):
        panes = [dict(p, tab_id=t, workspace_id="w1") for t, ps in self.tabs.items() for p in ps]
        return {"panes": panes, "focused_tab_id": self.focus[0], "focused_pane_id": self.focus[1]}

    def open_panel(self, target):
        self.n += 1
        pid = f"w1:p{self.n}"
        tab = self.tab_of(target)
        i = [p["pane_id"] for p in self.tabs[tab]].index(target)
        self.tabs[tab].insert(i + 1, {"pane_id": pid})
        self.argv[pid] = "panel"
        self.calls.append(("open", target))
        return {"pane_id": pid, "tab_id": tab}, None

    def swap(self, source, target):
        tab = self.tab_of(source)
        ps = self.tabs[tab]
        i, j = (next(k for k, p in enumerate(ps) if p["pane_id"] == x) for x in (source, target))
        ps[i], ps[j] = ps[j], ps[i]
        self.calls.append(("swap", source, target))
        self.focus = (tab, source)  # herdr focuses the swap's source and shows its tab
        self.log.record({"type": "tab_focused", "tab_id": tab})
        self.log.record({"type": "pane_focused", "pane_id": source})
        if self.on_swap:
            self.on_swap(self)
        return None

    def close(self, pid):
        tab = self.tab_of(pid)
        self.tabs[tab] = [p for p in self.tabs[tab] if p["pane_id"] != pid]
        self.calls.append(("close", pid))
        return None

    def focus_tab(self, tab):
        self.calls.append(("focus_tab", tab))
        self.focus = (tab, self.tabs[tab][0]["pane_id"])
        return None

    def focus_pane(self, pid):
        self.calls.append(("focus_pane", pid))
        self.focus = (self.tab_of(pid), pid)
        return None

    def order(self, tab):
        return [p["pane_id"] for p in self.tabs[tab]]


def orch(*panes):
    return SimpleNamespace(orchestrators=[SimpleNamespace(pane_id=p, machine=None) for p in panes])


def agent(pid):
    return {"pane_id": pid, "agent": "claude"}


@pytest.fixture
def world(monkeypatch):
    return World(monkeypatch, {"w1:t1": [{"pane_id": "w1:p1"}], "w1:t2": [agent("w1:p2")],
                               "w1:t3": [agent("w1:p3")], "w1:t4": [agent("w1:p4")]}, ("w1:t1", "w1:p1"))


def manager(world, tmp_path, verified=()):
    return panels.Manager(verified=lambda: set(verified), focus_log=world.log, state_path=tmp_path / "panels.json")


def test_the_start_batch_puts_a_panel_left_of_every_orchestrator_and_the_operator_back(world, tmp_path):
    m = manager(world, tmp_path)
    sw = orch("w1:p2", "w1:p3", "w1:p4")
    sv = m.reconcile(world.snapshot(), sw, start=True)
    for tab, ag in (("w1:t2", "w1:p2"), ("w1:t3", "w1:p3"), ("w1:t4", "w1:p4")):
        order = world.order(tab)
        assert len(order) == 2 and order[1] == ag and world.argv.get(order[0]) == "panel"  # panel on the left
        assert ("swap", ag, order[0]) in world.calls  # the agent is the swap's source
    assert world.focus == ("w1:t1", "w1:p1")  # the batch's own focus events did not count as the operator's
    assert world.calls[-2:] == [("focus_tab", "w1:t1"), ("focus_pane", "w1:p1")]
    assert m.panels_map(sv) == {t: world.order(t)[0] for t in ("w1:t2", "w1:t3", "w1:t4")}
    world.calls.clear()
    m.reconcile(world.snapshot(), sw)  # nothing missing: nothing done
    assert world.calls == []


def test_an_operator_who_moves_during_the_batch_is_left_there(world, tmp_path):
    world.tabs["w2:t9"] = [{"pane_id": "w2:p9"}]

    def operator_moves(w):  # during the batch the operator goes to a pane the batch never touches
        if len([c for c in w.calls if c[0] == "swap"]) == 2:
            w.log.record({"type": "pane_focused", "pane_id": "w2:p9"})
    world.on_swap = operator_moves
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2", "w1:p3", "w1:p4"), start=True)
    assert not any(c[0] in ("focus_tab", "focus_pane") for c in world.calls)


def test_after_the_start_only_the_operators_tab_gets_a_panel(world, tmp_path):
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2"), start=True)
    world.calls.clear()
    sw = orch("w1:p2", "w1:p3")  # a new orchestrator in t3; the operator is elsewhere
    m.reconcile(world.snapshot(), sw)
    assert world.calls == []
    world.focus = ("w1:t3", "w1:p3")  # they enter its tab, on the agent
    m.reconcile(world.snapshot(), sw)
    panel = world.order("w1:t3")[0]
    assert world.calls == [("open", "w1:p3"), ("swap", "w1:p3", panel)]
    assert world.focus == ("w1:t3", "w1:p3")  # still on the agent they are in


def test_a_lazy_panel_waits_on_the_right_when_the_operator_left_before_the_swap(world, tmp_path, monkeypatch):
    m = manager(world, tmp_path)
    m.started = True
    world.focus = ("w1:t2", "w1:p2")
    real_open = world.open_panel

    def open_then_leave(target):
        out = real_open(target)
        world.focus = ("w1:t1", "w1:p1")  # the operator switched away meanwhile
        return out
    monkeypatch.setattr(herdr, "open_panel", open_then_leave)
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert world.order("w1:t2")[0] == "w1:p2" and not any(c[0] == "swap" for c in world.calls)  # right, no pull
    world.focus = ("w1:t2", "w1:p2")  # next visit: swapped left then
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert world.order("w1:t2")[1] == "w1:p2" and ("swap", "w1:p2", world.order("w1:t2")[0]) in world.calls


def test_a_panel_the_operator_closed_stays_closed_until_the_open_key(world, tmp_path):
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    panel = world.order("w1:t2")[0]
    world.tabs["w1:t2"] = [agent("w1:p2")]  # the operator closed it
    world.focus = ("w1:t2", "w1:p2")
    world.calls.clear()
    m.reconcile(world.snapshot(), sw)
    m.reconcile(world.snapshot(), sw)
    assert world.calls == [] and m.closed == {"w1:t2"}
    again = manager(world, tmp_path)  # a restarted core remembers
    again.started = True
    again.reconcile(world.snapshot(), sw)
    assert world.calls == []
    assert again.open_key(world.snapshot(), sw) == "panel"
    new = world.order("w1:t2")[0]
    assert new != panel and world.focus == ("w1:t2", new) and again.closed == set()


def test_the_open_key_focuses_an_existing_panel_or_says_home(world, tmp_path):
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    world.focus = ("w1:t2", "w1:p2")
    assert m.open_key(world.snapshot(), sw) == "panel" and world.focus == ("w1:t2", world.order("w1:t2")[0])
    world.focus = ("w1:t3", "w1:p3")  # not an orchestrator of the model
    assert m.open_key(world.snapshot(), sw) == "home"


def test_only_provable_panels_are_closed(world, tmp_path):
    """A pane running our viewer that this core did not open and whose viewer never said a
    verified hello is never closed; it still counts as the tab's panel (no second one)."""
    world.tabs["w1:t2"].insert(0, {"pane_id": "w1:p50"})
    world.argv["w1:p50"] = "panel"
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2"), start=True)
    assert not any(c == ("open", "w1:p2") for c in world.calls)
    assert m.remove_all(world.snapshot()) == 0 and "w1:p50" in world.order("w1:t2")
    verified = manager(world, tmp_path, verified={"w1:p50"})
    assert verified.remove_all(world.snapshot()) == 1 and "w1:p50" not in world.order("w1:t2")


def test_a_tab_herdr_could_not_check_gets_no_panel(world, tmp_path, monkeypatch):
    world.tabs["w1:t2"].append({"pane_id": "w1:p60"})
    monkeypatch.setattr(herdr, "runs_panorama", lambda pane, cmd="tui", pid=None: None if pane == "w1:p60" else False)
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2"), start=True)
    assert world.calls == []


def test_a_second_panel_is_closed_keeping_the_connected_one(world, tmp_path):
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2"), start=True)
    first = world.order("w1:t2")[0]
    world.tabs["w1:t2"].append({"pane_id": "w1:p70"})
    world.argv["w1:p70"] = "panel"
    m.verified = lambda: {"w1:p70"}
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert ("close", first) in world.calls and "w1:p70" in world.order("w1:t2")


def test_a_tab_without_its_orchestrator_loses_the_panel_only_after_a_while(world, tmp_path):
    now = [1000.0]
    m = panels.Manager(verified=lambda: set(), focus_log=world.log, state_path=tmp_path / "p.json", clock=lambda: now[0])
    m.reconcile(world.snapshot(), orch("w1:p2"), start=True)
    panel = world.order("w1:t2")[0]
    m.reconcile(world.snapshot(), orch())  # Claude exited
    now[0] += panels.REMOVE_AFTER - 1
    m.reconcile(world.snapshot(), orch())
    assert panel in world.order("w1:t2")
    m.reconcile(world.snapshot(), orch("w1:p2"))  # started again in time: kept
    now[0] += panels.REMOVE_AFTER + 1
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert panel in world.order("w1:t2")
    m.reconcile(world.snapshot(), orch())
    now[0] += panels.REMOVE_AFTER
    m.reconcile(world.snapshot(), orch())
    assert panel not in world.order("w1:t2") and ("close", panel) in world.calls


def test_the_batch_guard_wraps_the_batch(world, tmp_path):
    """Q153's slot: one function around the start batch."""
    seen = []

    class Guard:
        def __enter__(self):
            seen.append(("in", len(world.calls)))

        def __exit__(self, *a):
            seen.append(("out", len([c for c in world.calls if c[0] == "swap"])))
    m = manager(world, tmp_path)
    m.guard = Guard
    m.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"), start=True)
    assert seen == [("in", 0), ("out", 2)]


# -- the core's open request and the viewers' switching -----------------------------------

@pytest.fixture
def paths():
    import shutil
    import tempfile
    from pathlib import Path
    base = Path(tempfile.mkdtemp(prefix="pm", dir="/tmp"))
    yield base / "s" / "core.sock", base / "s" / "core.lock"
    shutil.rmtree(base, ignore_errors=True)


def test_the_open_key_asks_the_core(paths):
    from herdr_panorama.panel import request_open
    sock, lock = paths
    assert request_open(sock, timeout=1) is None  # no core: the caller shows the home tab
    asked = []
    c = core.Core.start(sock, lock, on_open=lambda req: asked.append(req) or "panel")
    assert request_open(sock, timeout=3) == "panel" and asked == [{}]
    c.stop()


async def test_a_panel_switches_by_tab_and_panel_focus(paths, monkeypatch):
    from herdr_panorama.panel import PanelApp
    from .design_dock import FIX, design_swarm
    import json
    nine = json.loads((FIX / "design_9.json").read_text())
    sw = design_swarm(nine["data"])
    calls = []
    target = next(o for o in sw.orchestrators if o.tab_id != "w1:t3")
    snap = {"panes": [{"pane_id": target.pane_id, "tab_id": target.tab_id}, {"pane_id": "w1:p77", "tab_id": target.tab_id}]}
    monkeypatch.setattr(herdr, "snapshot", lambda: snap)
    monkeypatch.setattr(herdr, "focus_tab", lambda t: calls.append(("tab", t)))
    monkeypatch.setattr(herdr, "focus_pane", lambda p: calls.append(("pane", p)))
    monkeypatch.setattr(herdr, "focus_agent", lambda p: calls.append(("agent", p)))
    app = PanelApp(sock_path=paths[0], pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app.connected = True
        app._got_state(protocol.state_message(sw, {}, None, None, {target.tab_id: "w1:p77"})["state"])
        assert app.panes_of == {target.tab_id: "w1:p77"}
        app._dock(target.pane_id, keep_focus=True)
        for _ in range(40):
            await pilot.pause(0.02)
            if not app.docking:
                break
        assert calls == [("tab", target.tab_id), ("pane", "w1:p77")]
        calls.clear()
        app._dock(target.pane_id, keep_focus=False)  # Enter into the agent
        for _ in range(40):
            await pilot.pause(0.02)
            if not app.docking:
                break
        assert calls == [("agent", target.pane_id)]


def test_per_tab_keeps_the_home_panorama_home(monkeypatch):
    from herdr_panorama import config
    monkeypatch.setattr(home, "enabled", lambda: True)
    monkeypatch.setattr(config, "load", lambda: {"cockpit": True, "cockpit_panels": "per-tab"})
    assert cockpit.per_tab() and not cockpit.moving()
    monkeypatch.setattr(config, "load", lambda: {"cockpit": True})
    assert cockpit.moving() and not cockpit.per_tab()


def test_new_agent_builds_its_tab_with_the_panel_on_the_left(monkeypatch):
    monkeypatch.setattr(cockpit, "per_tab", lambda: True)
    monkeypatch.setattr(cockpit, "fit", lambda pane, tab, last=None: calls.append(("fit", pane)))
    calls = []
    monkeypatch.setattr(herdr, "agent_exists", lambda n: False)
    monkeypatch.setattr(herdr, "socket_call", lambda m, p, timeout=None: (calls.append((m, p)), (
        {"layout": {"tab_id": "w1:t9", "root": {"first": {"pane_id": "w1:p90"}, "second": {"pane_id": "w1:p91"}}}}, None))[1])
    monkeypatch.setattr(herdr, "start_agent", lambda n, p, **k: calls.append(("start", p)))
    monkeypatch.setattr(herdr, "focus_tab", lambda t: calls.append(("tab", t)))
    monkeypatch.setattr(herdr, "focus_pane", lambda p: calls.append(("pane", p)))
    monkeypatch.setenv("HERDR_PLUGIN_ROOT", "/plugins/panorama")
    name, err = home.new_agent("fresh", None, None)
    assert err is None and name == "fresh"
    method, params = calls[0]
    assert method == "layout.apply" and params["focus"] is False
    first, second = params["root"]["first"], params["root"]["second"]
    assert first["command"] == herdr.PANEL_COMMAND and first["env"]["HERDR_PLUGIN_ROOT"] == "/plugins/panorama"
    assert second == {"type": "pane"}
    assert calls[1:] == [("fit", "w1:p90"), ("start", "w1:p91"), ("tab", "w1:t9"), ("pane", "w1:p91")]
