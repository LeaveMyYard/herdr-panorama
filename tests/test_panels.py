"""Per-tab panels, part 2: the panel manager (panels.py) against an in-memory herdr: the start
batch and its focus restore, the operator's tab only afterwards, ownership, closed stays
closed, duplicates, orphans, the open key, the viewers' switching and + New agent."""

import json
import time
from types import SimpleNamespace

import pytest

from herdr_panorama import cockpit, core, herdr, home, panels, protocol

PANEL_LABEL = "Panorama panel"  # the manifest's title for the panel entrypoint (herdr labels the pane)


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
        # pane -> (plain shell?, shell age, has children): default an idle shell herdr just started
        self.shell: dict[str, tuple] = {}
        self.tab_focus: dict[str, str] = {}  # tab -> the pane it remembers as focused
        mp = monkeypatch
        mp.setattr(herdr, "snapshot", self.snapshot)
        mp.setattr(herdr, "open_panel", self.open_panel)
        mp.setattr(herdr, "swap_panes", self.swap)
        mp.setattr(herdr, "close_plugin_pane", self.close)
        mp.setattr(herdr, "close_pane", self.close_any, raising=False)
        mp.setattr(herdr, "pane_shell", self.pane_shell, raising=False)
        mp.setattr(herdr, "shell_state", self.shell_state, raising=False)
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
        snap = {"panes": panes, "focused_tab_id": self.focus[0], "focused_pane_id": self.focus[1]}
        if self.tab_focus:  # herdr's layouts: each tab's remembered focus
            snap["layouts"] = [{"tab_id": t, "focused_pane_id": f} for t, f in self.tab_focus.items()]
        return snap

    def open_panel(self, target):
        self.n += 1
        pid = f"w1:p{self.n}"
        tab = self.tab_of(target)
        i = [p["pane_id"] for p in self.tabs[tab]].index(target)
        self.tabs[tab].insert(i + 1, {"pane_id": pid, "label": PANEL_LABEL, "terminal_id": f"term_{self.n}"})
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

    def close_any(self, pid):
        tab = self.tab_of(pid)
        self.tabs[tab] = [p for p in self.tabs[tab] if p["pane_id"] != pid]
        self.calls.append(("close_pane", pid))
        return None

    def pane_shell(self, pid):
        if self.argv.get(pid):
            return False, 7000
        plain = self.shell.get(pid, (True, 3.0, False))[0]
        return (None, None) if plain is None else (plain, 7000 + int(pid.rsplit("p", 1)[1], 16))

    def shell_state(self, shell):
        pid = next(p for ps in self.tabs.values() for p in ps
                   if 7000 + int(p["pane_id"].rsplit("p", 1)[1], 16) == shell)["pane_id"]
        _, age, children = self.shell.get(pid, (True, 3.0, False))
        return age, children

    def restart_herdr(self):
        """herdr stopped unexpectedly and restored its session: every pane keeps its id and label
        but gets a new terminal running a plain login shell (no pane's command is started again)."""
        for ps in self.tabs.values():
            for p in ps:
                self.n += 1
                p["terminal_id"] = f"term_{self.n}"
        self.argv.clear()
        self.calls.clear()

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
    m.verified = lambda: {panel}
    m.reconcile(world.snapshot(), sw)  # its viewer connected
    m.verified = lambda: set()
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


# -- the review of #34 ------------------------------------------------------------------------

def test_a_viewer_exits_on_the_cores_shutdown(paths):
    """Medium: on quit or per-tab off the core tells its viewers to go; a viewer that exits
    closes its pane in herdr (plugin pane or layout.apply pane alike, checked on herdr 0.9.3)."""
    from herdr_panorama.panel import PanelClient
    sock, lock = paths
    c = core.Core.start(sock, lock)
    gone = []
    cl = PanelClient(sock, "w1:p9", "w1:t3", lambda st: None, lambda on: None, on_shutdown=lambda: gone.append(1)).start()
    for _ in range(100):
        if c.connected():
            break
        time.sleep(0.03)
    c.shutdown()
    for _ in range(100):
        if gone:
            break
        time.sleep(0.03)
    assert gone == [1]
    cl.stop()
    c.stop()


async def test_a_viewer_without_a_core_for_long_exits(paths, monkeypatch):
    """Medium: no core for NO_CORE_SECONDS (the design's 10 minutes): the viewer closes itself,
    so a crashed core never leaves panels behind for good."""
    from herdr_panorama import panel
    monkeypatch.setattr(panel, "NO_CORE_SECONDS", 0.3)
    monkeypatch.setattr(panel, "NO_CORE_CHECK", 0.05)
    app = panel.PanelApp(sock_path=paths[0], pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(40):
            await pilot.pause(0.05)
            if not app.is_running:
                break
        assert not app.is_running


async def test_a_connected_viewer_does_not_time_out(paths, monkeypatch):
    from herdr_panorama import panel
    monkeypatch.setattr(panel, "NO_CORE_SECONDS", 0.3)
    monkeypatch.setattr(panel, "NO_CORE_CHECK", 0.05)
    app = panel.PanelApp(sock_path=paths[0], pane_id="w1:p9", tab_id="w1:t3", start_client=False)
    async with app.run_test(size=(46, 45)) as pilot:
        app._status(True)
        await pilot.pause(0.6)
        assert app.is_running


def test_viewers_start_from_the_venv_not_uv_run():
    """Low: a panel's process is the venv's own entry point (uv run adds a ~31 MB parent per
    panel); uv run only when there is no venv."""
    cmd = herdr.PANEL_COMMAND[-1]
    assert cmd.index('.venv/bin/panorama" panel') < cmd.index("uv run")
    assert cmd.startswith('R="${HERDR_PLUGIN_ROOT:-.}"; [ -x "$R/.venv/bin/panorama" ] && exec ')


def test_a_panel_that_dies_before_its_viewer_connected_is_a_crash_not_a_close(world, tmp_path):
    """Low: a viewer that never said a verified hello (a broken environment) is not the
    operator closing it: no closed mark; it is made again, at most CRASH_LIMIT times."""
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    world.focus = ("w1:t2", "w1:p2")
    for _ in range(panels.CRASH_LIMIT + 2):
        panel = world.order("w1:t2")[0]
        if world.argv.get(panel) != "panel":
            break
        world.tabs["w1:t2"] = [agent("w1:p2")]  # it died
        m.reconcile(world.snapshot(), sw)
    assert m.closed == set()
    opens = [c for c in world.calls if c == ("open", "w1:p2")]
    assert len(opens) == panels.CRASH_LIMIT  # the first one and the re-makes, then it gives up
    assert "w1:t2" in m.broken


def test_a_panel_whose_viewer_connected_then_vanished_was_closed_by_the_operator(world, tmp_path):
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    panel = world.order("w1:t2")[0]
    m.verified = lambda: {panel}
    m.reconcile(world.snapshot(), sw)  # its viewer is connected
    world.tabs["w1:t2"] = [agent("w1:p2")]
    m.reconcile(world.snapshot(), sw)
    assert m.closed == {"w1:t2"}


def test_a_crash_marker_wins_over_a_connected_viewer(world, tmp_path, monkeypatch):
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp_path / "state"))
    from herdr_panorama import panel as viewer
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    p = world.order("w1:t2")[0]
    m.verified = lambda: {p}
    m.reconcile(world.snapshot(), sw)
    viewer.mark_crash(p)  # the viewer died of an exception after connecting
    world.tabs["w1:t2"] = [agent("w1:p2")]
    m.reconcile(world.snapshot(), sw)
    assert m.closed == set() and not viewer.crash_marker(p).exists()


def test_the_restore_skips_only_the_batchs_own_events_in_order(world, tmp_path):
    """Low: the operator going to a tab the batch also touched is still their own move: after
    the batch they are left there, not pulled back."""
    def operator_moves(w):
        if len([c for c in w.calls if c[0] == "swap"]) == 2:  # after t3's swap: back to t2, by hand
            w.log.record({"type": "tab_focused", "tab_id": "w1:t2"})
            w.log.record({"type": "pane_focused", "pane_id": "w1:p2"})
    world.on_swap = operator_moves
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), orch("w1:p2", "w1:p3", "w1:p4"), start=True)
    assert not any(c[0] in ("focus_tab", "focus_pane") for c in world.calls)


def test_a_known_panel_is_not_asked_about_on_every_pass(world, tmp_path, monkeypatch):
    """Low: a positive process-info answer is cached too (PANEL_SECONDS)."""
    world.tabs["w1:t2"].insert(0, {"pane_id": "w1:p50"})
    world.argv["w1:p50"] = "panel"
    asked = []
    monkeypatch.setattr(herdr, "runs_panorama", lambda pane, cmd="tui", pid=None: asked.append(pane) or world.argv.get(pane) == cmd)
    m = manager(world, tmp_path)
    for _ in range(3):
        m.reconcile(world.snapshot(), orch("w1:p2"), start=not m.started)
    assert asked.count("w1:p50") == 1


# -- after herdr stops unexpectedly (the operator's report, 2026-10-17) ---------------------------

def recorded_panel(world, tmp_path, tab="w1:t2"):
    """A core that made t2's panel and saw it running (so it is recorded); the panel."""
    m = manager(world, tmp_path)
    sw = orch("w1:p2")
    m.reconcile(world.snapshot(), sw, start=True)
    m.reconcile(world.snapshot(), sw)
    return m, world.order(tab)[0]


def restarted(world, tmp_path, passes=1, swarm=None):
    """The core after herdr's restart: a new manager, ``passes`` passes (no start batch, so
    only the closing is under test). Returns it."""
    again = manager(world, tmp_path)
    again.started = True
    for _ in range(passes):
        again.reconcile(world.snapshot(), swarm or orch("w1:p2", "w1:p3"))
    return again


def closes(world):
    return [c for c in world.calls if c[0] in ("close", "close_pane")]


def test_a_panel_herdr_restored_as_a_plain_shell_is_closed_and_made_again_in_its_place(world, tmp_path):
    """herdr's session restore brings a per-tab panel back as a plain shell with its old text
    (same pane id and label, a new terminal, no command). The restarted core closes it, as its
    record proves it was our panel, and the tab gets a fresh panel on the left of its agent."""
    sw = orch("w1:p2", "w1:p3")
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), sw, start=True)
    m.reconcile(world.snapshot(), sw)  # herdr's snapshot now shows the new panels: recorded
    old = {t: world.order(t)[0] for t in ("w1:t2", "w1:t3")}
    world.restart_herdr()  # herdr crashed and restored; the core (a herdr pane) went with it
    again = manager(world, tmp_path)
    again.reconcile(world.snapshot(), sw, start=True)
    assert ("close_pane", old["w1:t2"]) in world.calls and ("close_pane", old["w1:t3"]) in world.calls
    for tab, ag in (("w1:t2", "w1:p2"), ("w1:t3", "w1:p3")):
        order = world.order(tab)
        assert len(order) == 2 and order[1] == ag and world.argv.get(order[0]) == "panel"
        assert order[0] != old[tab]
    assert again.closed == set()  # closing a restored panel is not the operator closing it


@pytest.mark.parametrize("case", ["never-recorded", "same-terminal", "no-terminal-recorded", "pane-without-terminal",
                                  "relabelled", "agent", "focused", "focused-flag", "tab-focus",
                                  "in-use", "background-job", "children-unknown", "old-shell",
                                  "age-unknown", "quit-cleanly"])
def test_a_restored_pane_is_closed_only_when_provably_our_unused_panel(world, tmp_path, case):
    """Never close what is not provably ours and unused: a look-alike labelled like a panel, a
    pane whose terminal herdr did not replace, one relabelled, holding an agent, focused, with
    something running (a command, an exec'ed program, the rc) or a background job, a shell
    older than a fresh restore, and anything after a clean quit (no record survives one). Each
    is final: the same pane looking idle later is still left alone."""
    m, panel = recorded_panel(world, tmp_path)
    # the operator's own shell, labelled like a panel: never recorded
    world.tabs["w1:t4"].insert(0, {"pane_id": "w1:p60", "label": PANEL_LABEL, "terminal_id": "term_x"})
    if case == "never-recorded":  # e.g. a panel from before this fix
        m.record = {}
        m._save_closed()
    if case == "quit-cleanly":
        m.remove_all(world.snapshot())
        world.tabs["w1:t2"].insert(0, {"pane_id": panel, "label": PANEL_LABEL, "terminal_id": "term_y"})
    world.restart_herdr()
    pane = world.pane(panel)
    if case == "same-terminal":
        pane["terminal_id"] = m.record[panel]["terminal"]
    elif case == "no-terminal-recorded":
        again_rec = json.loads((tmp_path / "panels.json").read_text())
        again_rec["panes"][panel].pop("terminal")
        (tmp_path / "panels.json").write_text(json.dumps(again_rec))
    elif case == "pane-without-terminal":
        pane.pop("terminal_id")
    elif case == "relabelled":
        pane["label"] = "notes"
    elif case == "agent":
        pane["agent"] = "claude"
    elif case == "focused":
        world.focus = ("w1:t2", panel)
    elif case == "focused-flag":  # herdr's per-pane flag
        pane["focused"] = True
    elif case == "tab-focus":  # the operator was last in it (then went to another tab)
        world.tab_focus = {"w1:t2": panel}
    elif case == "in-use":
        world.shell[panel] = (False, 3.0, False)
    elif case == "background-job":
        world.shell[panel] = (True, 3.0, True)
    elif case == "children-unknown":
        world.shell[panel] = (True, 3.0, None)
    elif case == "old-shell":  # e.g. a core that starts late: the operator may have used it unseen
        world.shell[panel] = (True, panels.RESTORED_SECONDS + 1, False)
    elif case == "age-unknown":
        world.shell[panel] = (True, None, False)
    again = restarted(world, tmp_path)
    assert not closes(world)
    assert panel in world.order("w1:t2") and "w1:p60" in world.order("w1:t4")
    pane.pop("agent", None)
    pane.pop("focused", None)
    pane["label"] = PANEL_LABEL
    world.shell.pop(panel, None)
    world.focus = ("w1:t1", "w1:p1")
    world.tab_focus = {}
    again.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))  # idle now, still within the passes
    assert not closes(world)


def test_a_pane_focused_after_the_restore_is_never_closed(world, tmp_path):
    """The operator clicked into the restored panel (herdr's focus event) while herdr could not
    yet describe it: it is theirs now, even once it looks idle."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    world.shell[panel] = (None, None, None)
    again = restarted(world, tmp_path)
    world.log.record({"type": "pane_focused", "pane_id": panel})
    world.shell.pop(panel)
    again.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    assert not closes(world) and panel not in again.record


def test_a_restored_panel_is_judged_only_on_the_first_passes(world, tmp_path):
    """herdr cannot describe the pane for the first passes after the restore was seen: then it
    is given up on, and left as a shell (safe), not closed later."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    world.shell[panel] = (None, None, None)
    again = restarted(world, tmp_path, passes=panels.RESTORE_PASSES)
    assert panel in again.record
    world.shell.pop(panel)  # idle and describable from now on, but too late
    again.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    again.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    assert not closes(world) and panel not in again.record and panel in world.order("w1:t2")


@pytest.mark.parametrize("first,last", [(True, False), (False, True)], ids=["busy-at-the-last-look", "busy-first"])
def test_a_shell_seen_busy_at_either_look_is_not_closed(world, tmp_path, monkeypatch, first, last):
    """Two looks, one before ``ps`` and one right before the close: something running at
    either leaves the pane alone for good."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    looks = iter([(first, 7000), (last, 7000), (True, 7000), (True, 7000)])
    monkeypatch.setattr(herdr, "pane_shell", lambda pid: next(looks) if pid == panel else (True, 1))
    monkeypatch.setattr(herdr, "shell_state", lambda pid: (3.0, False))
    again = restarted(world, tmp_path, swarm=orch("w1:p2"))
    again.reconcile(world.snapshot(), orch("w1:p2"))
    assert ("close_pane", panel) not in world.calls


def test_a_live_handoff_keeps_the_panels_and_rekeys_the_record(world, tmp_path):
    """``herdr update --handoff`` gives every pane a new terminal id while its process lives
    on: a panel still running our viewer is never closed, and its record follows the new
    terminal."""
    m, panel = recorded_panel(world, tmp_path)
    for ps in world.tabs.values():
        for p in ps:
            p["terminal_id"] = p.get("terminal_id", "t") + "-handoff"
    world.calls.clear()
    m.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    m.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    assert not closes(world) and world.order("w1:t2")[0] == panel
    assert m.record[panel]["terminal"] == world.pane(panel)["terminal_id"]


def test_a_stale_record_never_closes_a_later_pane_with_the_same_id(world, tmp_path):
    """A fresh server (no session restored) numbers panes from the start again. The record's
    pane is missing on the first pass, so it is forgotten; a pane that later gets that id,
    even one labelled like a panel and idle, is not closed."""
    (tmp_path / "panels.json").write_text(json.dumps({"closed": [], "panes": {
        "w1:p50": {"tab": "w1:t2", "terminal": "term_old"}}}))
    m = manager(world, tmp_path)
    m.started = True
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert m.record == {}
    world.tabs["w1:t4"].insert(0, {"pane_id": "w1:p50", "label": PANEL_LABEL, "terminal_id": "term_new"})
    m.reconcile(world.snapshot(), orch("w1:p2"))
    assert not closes(world) and "w1:p50" in world.order("w1:t4")


def test_the_panel_record_is_kept_in_the_state_dir_with_herdrs_terminal(world, tmp_path):
    """The record that proves a restored pane was ours: pane id, tab and terminal id, saved
    next to the closed tabs, and emptied on a clean end."""
    sw = orch("w1:p2")
    m = manager(world, tmp_path)
    m.reconcile(world.snapshot(), sw, start=True)
    m.reconcile(world.snapshot(), sw)
    panel = world.order("w1:t2")[0]
    saved = json.loads((tmp_path / "panels.json").read_text())
    assert saved["panes"] == {panel: {"tab": "w1:t2", "terminal": world.pane(panel)["terminal_id"]}}
    m.remove_all(world.snapshot())
    assert json.loads((tmp_path / "panels.json").read_text())["panes"] == {}


@pytest.mark.parametrize("text,seconds", [("00:07", 7), ("01:02:03", 3723), ("2-00:00:01", 172801),
                                          (" 59:59\n", 3599), ("", None), ("x:1", None)],
                         ids=["mm:ss", "hh:mm:ss", "dd-hh:mm:ss", "padded", "empty", "garbage"])
def test_ps_elapsed_time_parses(text, seconds):
    assert herdr.PANEL_LABEL == PANEL_LABEL
    assert herdr._etime_seconds(text) == seconds


def test_shell_state_reads_ps_and_pgrep(monkeypatch):
    out = {"ps": (0, " 00:05\n"), "pgrep": (1, "")}

    def fake(*argv):
        code, text = out[argv[0]]
        return SimpleNamespace(returncode=code, stdout=text)
    monkeypatch.setattr(herdr, "_ps", fake)
    assert herdr.shell_state(42) == (5.0, False)
    out["pgrep"] = (0, "43\n")
    assert herdr.shell_state(42) == (5.0, True)
    out["pgrep"], out["ps"] = (2, ""), (1, "")
    assert herdr.shell_state(42) == (None, None)


# process-info as herdr 0.9.3 answered it on an isolated server (paths shortened): a live panel,
# whose ``sh -c`` execs panorama as the pane's shell pid, and the same pane after a crash and a
# session restore, a plain login shell. The others change only the process list.
LIVE_PANEL = {"foreground_process_group_id": 71225, "pane_id": "w1:p2", "shell_pid": 71225, "foreground_processes": [
    {"argv": ["/p/.venv/bin/python", "/p/.venv/bin/panorama", "panel"], "name": "panorama", "pid": 71225}]}
RESTORED = {"foreground_process_group_id": 73505, "pane_id": "w1:p2", "shell_pid": 73505, "foreground_processes": [
    {"argv": ["/usr/bin/zsh"], "name": "zsh", "pid": 73505}]}
LOGIN = dict(RESTORED, foreground_processes=[{"argv": ["-zsh"], "name": "zsh", "pid": 73505}])
BUSY = dict(RESTORED, foreground_process_group_id=73600, foreground_processes=[
    {"argv": ["vim", "notes"], "name": "vim", "pid": 73600}])
EXECED = dict(RESTORED, foreground_processes=[{"argv": ["vim", "notes"], "name": "vim", "pid": 73505}])
WITH_CHILD = dict(RESTORED, foreground_processes=[{"argv": ["/usr/bin/zsh"], "name": "zsh", "pid": 73505},
                                                  {"argv": ["sleep", "9"], "name": "sleep", "pid": 73700}])
NESTED = dict(RESTORED, foreground_processes=[{"argv": ["bash"], "name": "bash", "pid": 73800}])
EXEC_SCRIPT = dict(RESTORED, foreground_processes=[{"argv": ["bash", "deploy.sh"], "name": "bash", "pid": 73505}])
EXEC_C = dict(RESTORED, foreground_processes=[{"argv": ["zsh", "-c", "make"], "name": "zsh", "pid": 73505}])


@pytest.mark.parametrize("info,expect", [(RESTORED, (True, 73505)), (LOGIN, (True, 73505)),
                                         (LIVE_PANEL, (False, 71225)), (BUSY, (False, 73505)),
                                         (EXECED, (False, 73505)), (WITH_CHILD, (False, 73505)),
                                         (NESTED, (False, 73505)), (EXEC_SCRIPT, (False, 73505)),
                                         (EXEC_C, (False, 73505)),
                                         ({"pane_id": "w1:p2", "foreground_processes": []}, (None, None))],
                         ids=["restored", "login", "live-panel", "busy", "execed", "with-child", "nested-shell",
                              "exec-script", "exec-dash-c", "unknown"])
def test_a_plain_shell_is_only_the_panes_own_shell(monkeypatch, info, expect):
    monkeypatch.setattr(herdr, "call", lambda *a, **k: ({"type": "pane_process_info", "process_info": info}, None))
    assert herdr.pane_shell("w1:p2") == expect


def test_only_a_shell_seen_young_enough_is_closed(world, tmp_path):
    """The age bound is inclusive and short: a shell at ``RESTORED_SECONDS`` is closed."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    world.shell[panel] = (True, panels.RESTORED_SECONDS, False)
    restarted(world, tmp_path)
    assert ("close_pane", panel) in world.calls
    assert panels.RESTORED_SECONDS <= 15


def test_a_pane_used_before_the_cores_first_pass_is_never_closed(world, tmp_path):
    """The final review's scenario: right after the restart, before the core's first pass, the
    operator clicks into the restored panel, runs a command that finishes, and clicks the
    agent. The snapshot and the tab's focus now say "agent", the shell is idle and young, but
    the core's focus log (live from its start) saw the click: never closed."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    world.log = cockpit.FocusLog()  # the new core's stream, connected at its start
    world.log.live = True
    world.log.record({"type": "pane_focused", "pane_id": panel})
    world.log.record({"type": "pane_focused", "pane_id": "w1:p2"})
    world.focus = ("w1:t2", "w1:p2")
    world.tab_focus = {"w1:t2": "w1:p2"}
    again = restarted(world, tmp_path)
    assert not closes(world) and panel not in again.record


@pytest.mark.parametrize("stream", ["none", "not-live"])
def test_without_a_live_focus_stream_nothing_is_closed(world, tmp_path, stream):
    """Use of the pane could not be seen: the core decides nothing, and after its passes the
    pane is given up on (left as a shell)."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    if stream == "none":
        world.log = None
    else:
        world.log = cockpit.FocusLog()  # live stays False
    again = restarted(world, tmp_path, passes=panels.RESTORE_PASSES + 1)
    assert not closes(world) and panel not in again.record and panel in world.order("w1:t2")


def test_a_click_during_the_looks_stops_the_close(world, tmp_path, monkeypatch):
    """Focus is read again right before ``pane close``: a click into the pane while the core
    was asking about its processes leaves it alone."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    looks = []

    def pane_shell(pid):
        looks.append(pid)
        if len(looks) == 2:  # the operator clicks in while the core looks the second time
            world.log.record({"type": "pane_focused", "pane_id": panel})
        return True, 7000
    monkeypatch.setattr(herdr, "pane_shell", pane_shell)
    monkeypatch.setattr(herdr, "shell_state", lambda pid: (3.0, False))
    restarted(world, tmp_path, swarm=orch("w1:p2"))
    assert len(looks) == 2 and ("close_pane", panel) not in world.calls


def test_a_focus_stream_that_connects_within_the_passes_lets_the_core_decide(world, tmp_path):
    """No live stream yet is "ask again", not a verdict: once the stream is up (with no click on
    the pane), the restored panel is closed within the passes."""
    _, panel = recorded_panel(world, tmp_path)
    world.restart_herdr()
    world.log = cockpit.FocusLog()
    again = restarted(world, tmp_path)
    assert not closes(world) and panel in again.record
    world.log.live = True
    again.reconcile(world.snapshot(), orch("w1:p2", "w1:p3"))
    assert ("close_pane", panel) in world.calls
