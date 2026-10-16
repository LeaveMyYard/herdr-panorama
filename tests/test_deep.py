"""Subagents to any depth (design turn 5, direction 5d): one column per depth level, scrolled
sideways around the focus; the docked panel's spawner tree; the collector keeps whole trees.

The golden tests compare against docs/design/swarm-deep.js rendered by render-5d.mjs
(tests/fixtures/design_5d.json), cell for cell (chars, colours, bold) wherever the design draws
the new parts: the agent columns and the line above them. The orchestrator and thread columns
are 2a's, covered by the 2a tests."""

import dataclasses

import pytest

from herdr_panorama import render
from herdr_panorama.collect import Collector
from herdr_panorama.model import DONE, RUN, WAIT, Agent, Orchestrator, Swarm, Thread

from .design_deep import DEEP, NOW, TID, deep_swarm
from .test_scan_session import SID, M, agent, load, session, write
from .test_tui import disarm, loaded, make_app, screen_text

FRAMES = DEEP["frames"]
DEPTH = {"b1": 2, "c1": 4}


def norm(ch, fg, bg, bold):
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def gold_row(frame: str, key: str, y: int) -> list:
    return [norm(ch, r[1], r[2], r[3]) for r in FRAMES[frame][key][y] for ch in r[0]]


def view_for(sel: str | None, frame: int = 0, **kw) -> render.View:
    return render.View(orch="w1:p3", thread=TID, agent=sel, now=NOW, frame=frame,
                       focus=1 if sel is None else 1 + DEPTH[sel], **kw)


@pytest.mark.parametrize("frame", sorted(FRAMES))
@pytest.mark.parametrize("key,sel,x0", [("V4", None, 89), ("V5", "b1", 0), ("V6", "c1", 0)])
def test_5d_matches_the_design_cell_for_cell(frame, key, sel, x0):
    """V4: focus on the thread (today's 2a, depth columns to the right); V5: on b1 at depth 2;
    V6: on c1 at depth 4. Rows 2..41: the line above the columns and the columns."""
    scr = render.render(deep_swarm(), view_for(sel, int(frame)), 160, 45)
    diffs = []
    for y in range(2, 42):
        want, got = gold_row(frame, key, y), [norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y]]
        diffs += [(x, y, want[x], got[x]) for x in range(x0, 160) if want[x] != got[x]]
    assert not diffs, f"{len(diffs)} cells differ, first: {diffs[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES))
def test_docked_panel_tree_rows_match_the_design(frame):
    """The thread row and the indented tree of the agents that spawned others (x >= 5: the
    design's thread is not the last one, so its trunk column differs)."""
    sw = deep_swarm()
    sw.docked = "w1:p3"
    scr = render.render(sw, render.View(orch="w1:p3", dock=True, now=NOW, frame=int(frame)), 46, 45)
    for y in range(4, 10):
        want = [norm(ch, r[1], r[2], r[3]) for r in FRAMES[frame]["P4"][y] for ch in r[0]][5:46]
        got = [norm(c.ch, c.fg or render.FG, c.bg, c.bold) for c in scr.c[y][5:46]]
        assert want == got, (y, "".join(c[0] for c in got))
    assert "".join(c.ch for c in scr.c[4][:5]) == "▌  └ "  # the only thread: its row closes the list


# -- layout rules -------------------------------------------------------------------------

def test_nav_has_one_column_per_depth_in_screen_order():
    sw = deep_swarm()
    cols = render.nav(sw, view_for("c1"))
    assert cols[2] == ["l1", "v1"]
    assert cols[3] == ["r1", "b1", render.FIN_CARD + "l1"]
    assert cols[4] == ["r2", "f1", render.FIN_CARD + "r1", "s1", "s2", "s6", "s7", "s8", render.FIN_CARD + "b1"]
    assert cols[5] == ["c1"] and cols[6] == ["p1", render.FIN_CARD + "c1"] and len(cols) == 7
    v = view_for("c1", fin_open={"b1"})
    assert render.nav(sw, v)[4][-4:] == [render.FIN_CARD + "b1", "s3", "s4", "s5"]  # unfolded after its card


def test_columns_keep_their_width_and_align_left():
    """Operator: a column never stretches into the space left on the right (a stretched card's
    sweep ran too long); today's 2a agent column too."""
    sw = deep_swarm()
    rows = render.render(sw, view_for(None), 160, 45).text().split("\n")
    assert rows[4][93] == "┨" and rows[4][136] == "│" and rows[4][137:].strip() == ""  # 44 wide from x=93
    rows = render.render(sw, view_for("b1"), 160, 45).text().split("\n")
    assert [rows[3][x] for x in (4, 47, 55, 98, 106, 149)] == ["╭", "╮", "╭", "╮", "╭", "╮"]
    assert rows[3][150:].strip() == ""


def test_narrow_screens_shrink_the_last_column_and_keep_the_focus_and_its_parents():
    sw = deep_swarm()
    scr = render.render(sw, view_for("c1"), 120, 45)
    rows = scr.text().split("\n")
    assert "‹ Core Library › Refactor: plugin loader › lead l1 › researcher r1   4 columns left" in rows[2]
    assert "depth 5  1 column right ›" in rows[2]  # c1's children did not fit
    assert "checker c1" in scr.text() and "researcher r2" in rows[4]
    x = [c for c, _ in enumerate(rows[3]) if rows[3][c] == "╮"]
    assert x and x[-1] < 118


def test_a_flat_thread_draws_like_2a_without_the_scroll_line():
    """No nested agents: orchestrators | threads | agents, nothing off-screen, no extra key."""
    from .demo import NOW as DNOW, demo_swarm
    scr = render.render(demo_swarm(), render.View(orch="w1:p1", thread="ref:#42", agent="a2f81c0", now=DNOW,
                                                  focus=2), 160, 45)
    rows = scr.text().split("\n")
    assert rows[2].strip() == "" and "shift+wheel" not in rows[44]
    assert rows[4][85] == "├" and rows[4][93] == "┤"


def test_a_parent_loop_or_a_missing_parent_never_hides_an_agent():
    t = Thread(id="t", name="loop", source="turn",
               agents=[Agent(id="a", parent="b", status=RUN), Agent(id="b", parent="a", status=RUN),
                       Agent(id="c", parent="gone", status=RUN), Agent(id="d", parent="a", status=RUN)])
    kids = render.tree_kids(t)
    shown = {a.id for v in kids.values() for a in v}
    assert shown == {"a", "b", "c", "d"} and {a.id for a in kids[None]} >= {"c"}
    assert {e.ident for e in render.tree_entries(t, render.View())} == {"a", "b", "c", "d"}


def test_needs_filter_keeps_the_path_to_an_agent_that_needs_you():
    sw = deep_swarm()
    cols = render.nav(sw, view_for("c1", filter="needs"))
    assert cols[2] == ["l1"] and cols[3] == ["r1", "b1"] and "c1" in cols[5]
    assert "v1" not in cols[2] and "s1" not in cols[4]  # nothing below them needs you


# -- keys and mouse -------------------------------------------------------------------------

def deep_app():
    return make_app(deep_swarm(), focus_pane="w1:p3")[0]


async def test_arrows_walk_the_tree_and_scroll_sideways(fake_herdr):
    app = deep_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        v = app.view
        await pilot.press("right", "right")
        assert (v.focus, v.thread, v.agent) == (2, TID, "l1")
        await pilot.press("right")
        assert (v.focus, v.agent) == (3, "r1")
        await pilot.press("down")
        assert v.agent == "b1"
        await pilot.press("right")
        assert (v.focus, v.agent) == (4, "s1")
        text = screen_text(app)
        assert "‹ Core Library › Refactor: plugin loader › lead l1   3 columns left" in text
        await pilot.press("left")
        assert (v.focus, v.agent) == (3, "b1")  # up to the agent that spawned it
        await pilot.press("right")
        assert v.agent == "s1"  # the child last selected there
        await pilot.press("left", "left")
        assert (v.focus, v.agent) == (2, "l1")
        await pilot.press("left")
        assert v.focus == 1 and v.agent == "l1"
        await pilot.press("right")
        assert (v.focus, v.agent) == (2, "l1")


async def test_right_on_a_leaf_or_a_folded_card_does_nothing(fake_herdr):
    app = deep_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app._point("w1:p3", TID, "v1")
        await pilot.press("right")
        assert (app.view.focus, app.view.agent) == (2, "v1")
        app._point("w1:p3", TID, render.FIN_CARD + "l1")
        assert app.view.focus == 3
        disarm(app)
        await pilot.press("enter")
        assert "l1" in app.view.fin_open  # Enter unfolds a nested group's finished agents
        await pilot.press("right")
        assert app.view.focus == 3


async def test_clicks_on_deep_cards_focus_their_column(fake_herdr):
    app = deep_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app._point("w1:p3", TID, "r2")
        await pilot.pause()
        assert app.view.focus == 4
        app.arrived.clear()
        app.click_target("agent", ("w1:p3", TID, "f1"), cell=(0, 0))
        assert (app.view.focus, app.view.agent) == (4, "f1")
        disarm(app)
        app.click_target("fin", ("w1:p3", TID, "r1"), cell=(0, 0))
        assert app.view.agent == render.FIN_CARD + "r1" and "r1" in app.view.fin_open


async def test_shift_wheel_scrolls_sideways_and_a_focus_change_scrolls_back(fake_herdr):
    app = deep_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app._point("w1:p3", TID, "b1")
        app.repaint()
        assert app.view.hlo == 2
        app.hwheel(2)
        assert app.view.hcol == 4 and app.view.agent == "b1"  # the focus stays
        assert "‹ Core Library › Refactor: plugin loader › lead l1 › batch b1   4 columns left" in screen_text(app)
        app.hwheel(-9)
        assert app.view.hcol == 0
        await pilot.press("down")
        assert app.view.hcol is None and app.view.hlo == 2


def test_grid_maps_shift_wheel_and_the_horizontal_wheel_to_sideways():
    from herdr_panorama.tui import Grid

    calls = []

    class App:
        def hwheel(self, n):
            calls.append(("h", n))

        def wheel(self, col, n):
            calls.append(("v", col, n))

    class Ev:
        def __init__(self, shift=False):
            self.x, self.y, self.shift = 60, 10, shift  # on b1's column

        def stop(self):
            pass

    g = Grid.__new__(Grid)
    g.screen_data = render.render(deep_swarm(), view_for("b1"), 160, 45)
    object.__setattr__(g, "_test_app", App())
    Grid.app = property(lambda self: self._test_app)
    try:
        g.on_mouse_scroll_down(Ev(shift=True))
        g.on_mouse_scroll_up(Ev())
        g.on_mouse_scroll_right(Ev())
        g.on_mouse_scroll_left(Ev())
    finally:
        del Grid.app
    assert calls == [("h", 1), ("v", 2, -3), ("h", 1), ("h", -1)]


# -- the collector keeps whole trees ------------------------------------------------------

def test_collector_nests_any_depth_in_one_thread_and_keeps_finished_ancestors(tmp_path, monkeypatch):
    """main -> a1 -> d2 -> ... -> d10 (depth 10; the old root walk stopped after 8 hops). a1
    finished long ago (outside the done window) while the agents it spawned still run: it
    stays, so the tree is whole."""
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(tmp_path / "events"))
    proj, m = session(tmp_path)
    t0 = 1e9
    call = m.launch(t0, "a1", "Lead the refactor")
    a1 = agent(proj, "a1", "Lead the refactor", toolUseId=call)
    a1.prompt(t0, "brief")
    files, parent, pcall = [m, a1], a1, None
    for depth, aid in enumerate([f"d{i}" for i in range(2, 11)], start=2):
        pcall = parent.launch(t0 + depth * M, aid, f"level {depth}", model="haiku")
        child = proj.agent(SID, aid, {"description": f"level {depth}", "spawnDepth": depth, "toolUseId": pcall},
                           kind="meta_nested")
        child.prompt(t0 + depth * M, "brief")
        files.append(child)
        parent = child
    m.notify(t0 + 6 * M, "a1", call, "completed")
    parent.bash(t0 + 59 * M, "still working", pending=True)
    write(*files)
    sess = load(proj)
    c = Collector(root=str(tmp_path / "projects"), now=lambda: t0 + 60 * M, done_window=10 * M)
    threads_, _ = c._threads(sess, {}, t0 + 60 * M)
    chain = ["a1"] + [f"d{i}" for i in range(2, 11)]
    t = next(t for t in threads_ if any(a.id == "d10" for a in t.agents))
    by = {a.id: a for a in t.agents}
    assert set(chain) <= set(by)  # one thread holds the whole chain
    assert by["a1"].status == DONE and by["d10"].status == RUN
    assert [by[i].parent for i in chain[1:]] == chain[:-1]
    assert render.path_ids(t, "d10") == chain and render.agent_depth(t, "d10") == 10


def test_status_glyph_of_a_waiting_deep_agent_reaches_the_orchestrator():
    sw = deep_swarm()
    o = sw.orchestrators[0]
    assert any(a.status == WAIT for a in o.agents())
    assert render.orch_state(o, 0).st in ("fail", "wait")


async def test_a_click_right_after_a_sideways_scroll_does_not_hit_what_slid_under_the_pointer(fake_herdr):
    """Review M: trackpad momentum keeps scrolling while the operator clicks. A cell whose card
    changed in a scroll ignores clicks for SCROLL_ARM_SECONDS; one that did not still works."""
    import time as _time

    from herdr_panorama import tui
    app = deep_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app._point("w1:p3", TID, "b1")
        app.repaint()
        app.arrived.clear()
        before = app._cells()
        app.hwheel(1)
        after = app._cells()
        moved = next(c for c, hit in after.items() if hit[0] == "agent" and before.get(c) != hit)
        same = next((c for c, hit in after.items() if before.get(c) == hit and hit[0] == "agent"), None)
        target = after[moved]
        app.click_target(*target, cell=moved)
        assert app.view.agent == "b1"  # ignored: the card slid there in this scroll
        if same is not None:
            app.click_target(*after[same], cell=same)
            assert app.view.agent == after[same][1][2]
        app.scrolled = {c: _time.monotonic() - 0.01 for c in app.scrolled}  # the window passed
        app.click_target(*target, cell=moved)
        assert app.view.agent == target[1][2]
        assert tui.SCROLL_ARM_SECONDS < tui.CLICK_ARM_SECONDS
