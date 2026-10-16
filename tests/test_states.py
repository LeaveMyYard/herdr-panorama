"""Waiting and stale (design turn 8): golden tests against docs/design/swarm-states.js rendered
by render-8.mjs (tests/fixtures/design_8.json), and the state rules behind them."""

import json

import pytest

from herdr_panorama import render
from herdr_panorama.model import BG, DONE, Agent, BgTask, Orchestrator, Swarm, Thread

from .design_dock import FIX, design_swarm

DESIGN = json.loads((FIX / "design_8.json").read_text())
FRAMES = DESIGN["frames"]
NOW = 1_800_000_000.0
NOW_MIN = 10 * 60 + 4  # the design's clock, 10:04
H = 56
FLAVOUR = {"co": "coordinator", "pr": "pr-review", "sp": "superpowers"}  # swarm-tui.js FL keys


def minutes(hhmm: str) -> int:
    h, m = hhmm.split(":")
    return (NOW_MIN - int(h) * 60 - int(m)) % 1440


def states_swarm() -> Swarm:
    """The design's turn-8 dataset: the earlier sample plus four main-session-only
    orchestrators (two waiting, two stale) and PR #214's author n1 waiting on a CI watch."""
    sw = design_swarm()
    data = DESIGN["data"]
    by_tab = {o["tab"]: o for o in data["orchs"]}
    for o in sw.orchestrators:
        o.last_activity = NOW - by_tab[o.tab_no]["last"] * 60
        o.flavour = FLAVOUR[by_tab[o.tab_no]["fl"]]
    for d in data["orchs"]:
        if d["tab"] <= len(sw.orchestrators):
            continue
        o = Orchestrator(pane_id=f"w1:p{d['tab']}", tab_id=f"w1:t{d['tab']}", tab_label=d["name"], session_id=None,
                         herdr_status="idle", tab_no=d["tab"], flavour=FLAVOUR[d["fl"]])
        o.last_activity = NOW - d["last"] * 60
        for i, b in enumerate(d["bg"]):
            age = d["bgAge"] if d["bgAge"] else minutes(b["since"])
            o.bg.append(BgTask(f"b{d['tab']}{i}", b["kind"], b["desc"], NOW - age * 60))
        sw.orchestrators.append(o)
    n1 = next(a for o in sw.orchestrators for a in o.agents() if a.id == "n1")
    d1 = data["n1"]
    n1.status, n1.activity, n1.last_message, n1.tool_calls = BG, d1["act"], d1["msg"], d1["tools"]
    n1.phases, n1.started = list(d1["phs"]), NOW - 74 * 60  # "1h14m"
    n1.bg = [BgTask("n1b", b["kind"], b["desc"], NOW - minutes(b["since"]) * 60) for b in d1["bg"]]
    sw.docked = "w1:p5"
    return sw


def norm(ch, fg, bg, bold):
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def grid(runs):
    return [[norm(ch, r[1], r[2], r[3]) for r in row for ch in r[0]] for row in runs]


def cells(scr, w):
    return [[norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y][:w]] for y in range(scr.h)]


def diff(want, got, xs, ys):
    return [(x, y, want[y][x], got[y][x]) for y in ys for x in xs if want[y][x] != got[y][x]]


def graph(frame, **kw):
    v = render.View(focus=2, frame=frame, now=NOW, **kw)
    return cells(render.render(states_swarm(), v, 160, H), 160)


ORCH_X = range(2, 30)  # the orchestrator cards
# Rows compared there: the new cards (tabs 5-8, rows 23-42) whole. The earlier four keep what the
# app already drew before turn 8 (the app gives a name 4 more cells than the design's 2a), so
# only their state lines are compared.
ORCH_Y = [5, 10, 15, 20, *range(23, 43)]

@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_3a_panel_is_the_design_cell_for_cell(frame):
    v = render.View(orch="w1:p5", dock=True, frame=int(frame), now=NOW)
    v.hover = ("orch", ("w1:p7",))  # the design's hover
    got = cells(render.render(states_swarm(), v, 46, H), 46)
    d = diff(grid(FRAMES[frame]["P"]), got, range(46), range(H))
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_2a_orchestrators_and_background_card_are_the_design(frame):
    want = grid(FRAMES[frame]["A"])
    got = graph(int(frame), orch="w1:p5", thread=render.BG_CARD + "w1:p5")
    d = diff(want, got, ORCH_X, ORCH_Y) + diff(want, got, range(36, 86), range(3, 8))
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_2a_waiting_agent_card_is_the_design(frame):
    want = grid(FRAMES[frame]["B"])
    got = graph(int(frame), orch="w1:p4", thread="p214", agent="n1")
    d = diff(want, got, ORCH_X, ORCH_Y) + diff(want, got, range(93, 137), range(3, 13))
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_running_filter_keeps_waiting_and_drops_stale_like_the_design(frame):
    want = grid(FRAMES[frame]["R"])
    got = graph(int(frame), orch="w1:p4", thread="p214", agent="n1", filter="running")
    d = diff(want, got, ORCH_X, [5, 10, 15, 20, *range(23, 33)])  # 6 cards: stale ones are gone
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


# -- the rules ---------------------------------------------------------------------------

def orch(**kw) -> Orchestrator:
    return Orchestrator(pane_id="p", tab_id="t", tab_label="o", session_id="s", **{"herdr_status": "idle", **kw})


def view(**kw) -> render.View:
    return render.View(now=NOW, stale_after=12 * 3600, **kw)


def test_idle_turns_stale_after_stale_after():
    assert render.orch_state(orch(last_activity=NOW - 11 * 3600), 0, view()).st != "stale"
    st = render.orch_state(orch(last_activity=NOW - 13 * 3600), 0, view())
    assert (st.st, st.label, st.quiet, st.attention) == ("stale", "stale 13h", True, False)
    assert render.orch_state(orch(last_activity=NOW - 50 * 3600), 0, view()).label == "stale 2d"


def test_waiting_wins_over_stale_and_says_its_age_once_older_than_stale_after():
    young = orch(last_activity=NOW - 13 * 3600, bg=[BgTask("b", "Bash", "watch", NOW - 600)])
    assert render.orch_state(young, 0, view()).label == "waiting"
    old = orch(last_activity=NOW - 13 * 3600, bg=[BgTask("b", "Bash", "watch", NOW - 15 * 3600)])
    st = render.orch_state(old, 0, view())
    assert (st.st, st.label, st.bg, st.attention) == ("bg", "waiting 15h", 1, False)


def test_working_and_attention_win_over_waiting():
    o = orch(bg=[BgTask("b", "Bash", "watch", NOW)], herdr_status="working")
    assert render.orch_state(o, 0, view()).st == "run"
    o = orch(bg=[BgTask("b", "Bash", "watch", NOW)], inbox=[3])
    assert render.orch_state(o, 0, view()).st == "wait"


def test_all_done_threads_go_stale_too():
    t = Thread(id="t", name="t", source="turn", agents=[Agent(id="a", status=DONE)], status="done")
    assert render.orch_state(orch(threads=[t], last_activity=NOW - 60), 0, view()).st == "done"
    assert render.orch_state(orch(threads=[t], last_activity=NOW - 13 * 3600), 0, view()).st == "stale"


def test_needs_me_shows_neither_waiting_nor_stale():
    sw = Swarm(orchestrators=[orch(bg=[BgTask("b", "Bash", "w", NOW)]), orch(last_activity=NOW - 20 * 3600)])
    assert render.visible_orchs(sw, view(filter="needs")) == []
    assert len(render.visible_orchs(sw, view(filter="running"))) == 1


def test_the_clock_turns_once_a_second_and_keeps_frames_running():
    assert [render.clock(f) for f in (0, 3, 4, 8, 28, 32)] == ["⠁", "⠁", "⠈", "⠐", "⠂", "⠁"]
    assert all(0x2800 <= ord(c) <= 0x28FF for c in render.CLOCK)  # braille: drawn cell-exact (ghostty sprites)
    assert render.animated(Swarm(orchestrators=[orch(bg=[BgTask("b", "Bash", "w", NOW)])]))
    assert not render.animated(Swarm(orchestrators=[orch(last_activity=NOW - 20 * 3600)]))


def test_the_background_card_is_selectable_in_the_thread_column():
    sw = Swarm(orchestrators=[orch(bg=[BgTask("b", "Bash", "w", NOW)])])
    cols = render.nav(sw, view(orch="p"))
    assert cols[1][0] == render.BG_CARD + "p"


async def test_the_background_card_is_reached_with_right_and_enter_does_nothing_else():
    from .test_tui import disarm, loaded, make_app
    sw = states_swarm()
    sw.generated_at = NOW
    app, _ = make_app(sw, focus_pane="w1:p5")
    async with app.run_test(size=(160, H)) as pilot:
        await loaded(app, pilot)
        app.view.orch = "w1:p5"
        disarm(app)
        await pilot.press("right")
        assert app.view.focus == 1 and app.view.thread == render.BG_CARD + "w1:p5"
        disarm(app)
        await pilot.press("enter")
        await pilot.pause()
        assert app.view.focus == 1  # no agents behind it: Enter stays
        assert "background  2 tasks · main session" in render.render(sw, app.view, 160, H).text()


def test_the_waiting_clock_steps_on_the_restyle_path():
    """The clock is an Anim like the spinner: a screen drawn at frame 0 and restyled to frame 4 shows
    the next clock step with no re-render."""
    v = render.View(orch="w1:p5", thread=render.BG_CARD + "w1:p5", focus=2, frame=0, now=NOW)
    scr = render.render(states_swarm(), v, 160, H)
    at = lambda s: s.c[4][38].ch  # noqa: E731 (the background card's title clock)
    assert at(scr) == render.CLOCK[0]
    render.restyle(scr, 4.0)
    assert at(scr) == render.CLOCK[1]


@pytest.mark.parametrize("w", [160, 240, 320])
def test_the_background_card_takes_the_wide_thread_column(w):
    """Turn 7's wider cards: the background card is as wide as the thread cards below it."""
    sw = states_swarm()
    sw.orchestrators[3].bg = [BgTask("x", "Bash", "a long description " * 4, NOW - 60)]
    v = render.View(orch="w1:p4", thread="p214", agent="n1", focus=2, now=NOW)
    text = render.render(sw, v, w, H).text().splitlines()
    right = lambda word: next(r.index("│", r.index(word)) for r in text if word in r)  # noqa: E731
    assert right("background") == right("PR #212")
