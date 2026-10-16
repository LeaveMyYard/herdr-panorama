"""The 2a renderer on demo model data: both layouts, as text."""

from herdr_panorama import render
from herdr_panorama.render import DONE_CARD, FIN_CARD, View, nav

from .demo import NOW, demo_swarm


def view(**kw) -> View:
    v = View(orch="w1:p1", thread="ref:#42", agent="a2f81c0", now=NOW)
    for k, val in kw.items():
        setattr(v, k, val)
    return v


def lines(scr) -> list[str]:
    return scr.text().split("\n")


def test_wide_layout_has_the_three_columns_and_links():
    scr = render.render(demo_swarm(), view(), 160, 45)
    rows = lines(scr)
    assert len(rows) == 45 and all(len(r) <= 160 for r in rows)
    assert rows[0].startswith(" herdr panorama    all   running   needs me")
    assert rows[0].rstrip().endswith("? 1  ! 1  ✗ 1    10:04")
    # orchestrator cards at x=2, width 28
    assert rows[3][2:30] == "╭" + "─" * 26 + "╮"
    assert rows[4][2:5] == "┃  " and "✗  Billing API" in rows[4] and "?1" in rows[4][:31]  # state pod, edge
    assert "failed · working" in rows[5]  # state label (herdr says the main session works)
    assert rows[6][2:30].endswith("─ pr-review ─╯")
    assert "!  Docs site" in rows[14] and "blocked" in rows[15]
    # selected thread unfolds into its steps with times
    assert "» PR #42 invoice export" in rows[4]
    assert "review r2 · 21m" in rows[5]
    text = scr.text()
    assert "✓ author       09:02–09:20" in text
    assert "» review r2    since 09:43 · 21m   0B 1H 2M" in text
    assert "· CI" in text and "· merge" in text
    # other active threads collapse; done threads fold into one card
    assert "✗ PR #44 health badge" in text and "? PR #48 retry banner copy" in text
    assert "✓ 1 complete  PR #40 currency codes" in text
    # agents column: selected agent expanded, live agents as cards, finished folded
    assert "reviewer a2f81c0 · opus" in text and "reviewing" in text
    assert "now  Bash(Diff totals.py against roun…" in text  # the agent card keeps its 44 columns
    assert "g open pane   t transcript   a answer" in text
    assert "author a1c3d90 · opus" in text
    assert "3 finished · explorer ×3" in text
    # dashed links between the columns
    # solid links: the borders become tees, the trunk branches
    assert rows[4][29] == "├" and rows[4][32] == "┬" and rows[4][36] == "┨"  # into an edged card
    assert rows[4][85] == "├" and rows[4][89] == "┬" and rows[4][93] == "┤"
    # legend and keys
    assert "✓  done   ✓? done?   ⠋  running   »  review" in rows[42] and "~  lost" in rows[42]
    assert rows[44].startswith(" ↑↓ move   → open   ← back   enter details   g go to pane   a answer   i inbox")


def test_narrow_layout_is_an_indented_list_with_only_the_selected_orchestrator_open():
    scr = render.render(demo_swarm(), view(orch="w1:p2", thread="wf:wf_search", agent="j1a2b3c"), 70, 45)
    rows = lines(scr)
    assert all(len(r) <= 70 for r in rows)
    text = scr.text()
    assert "✗  Billing API  failed · working" in text and "✗ ?1" in rows[2]
    assert "Search refactor  working  workflow" in text
    assert "!  Docs site  blocked" in text and "✓  Release notes  all done" in text
    assert rows[5].startswith(" │") and rows[7].startswith(" ╰─┨")  # solid tree line into the edged card
    assert "search-index-rebuild" in text and "Build · 22m" in text
    assert "✓ Plan         09:20–09:41" in text
    assert "author j1a2b3c · sonnet" in text and "now  Read(src/search/tokenize.rs)" in text
    assert "✓ done ✓? done? ⠋ run » rev ○ queue ? ask ! block ✗ fail ~ lost" in text
    assert rows[44].startswith(" ↑↓ move   → open   g pane   a answer   f filter   ? help")


def test_question_and_blocked_detail_lines():
    sw = demo_swarm()
    text = render.render(sw, view(thread="ref:#48", agent="c1d2e3f"), 160, 45).text()
    assert "? Use \"Retrying…\" or \"Reconnecting…\"" in text
    assert "  for the banner? The design doc and" in text
    assert "needs answer" in text
    text = render.render(sw, view(orch="w1:p3", thread="main:w1:p3", agent="main:w1:p3"), 160, 45).text()
    assert "! Publish the v2 docs now or after the" in text and "  release?" in text
    assert "g open pane  t transcript  a answer    │" in text  # the hints fit the card


def test_explicit_thread_shows_pct_and_worktree():
    text = render.render(demo_swarm(), view(orch="w1:p3", thread="x:docs-v2", agent="d1e2f3a"), 160, 45).text()
    assert "write · 33m · 60%" in text
    assert "wt/docs-v2" in text and "writer d1e2f3a · sonnet" in text


def test_nav_lists_identities_per_column():
    sw = demo_swarm()
    cols = nav(sw, view())
    assert cols[0] == ["w1:p1", "w1:p2", "w1:p3", "w1:p4", render.NEW_ROW]  # the + row is always last
    assert cols[1] == ["ref:#42", "ref:#44", "ref:#48", DONE_CARD + "w1:p1"]
    assert cols[2] == ["a1c3d90", "a2f81c0", FIN_CARD + "ref:#42"]  # live by start time, then the folded card
    v = view(done_open={"w1:p1"}, fin_open={"ref:#42"})
    cols = nav(sw, v)
    assert cols[1][-1] == "ref:#40" and cols[2][-3:] == ["a3e1b22", "a4d0e11", "a5b7f02"]


def test_filters():
    sw = demo_swarm()
    assert nav(sw, view(filter="needs"))[0] == ["w1:p1", "w1:p3", render.NEW_ROW]
    assert nav(sw, view(filter="needs"))[1] == ["ref:#44", "ref:#48"]
    assert nav(sw, view(filter="running"))[1] == ["ref:#42", "ref:#44", "ref:#48"]
    assert "needs me" in render.render(sw, view(filter="needs"), 160, 45).text()


def test_untrusted_text_is_stripped_of_control_characters():
    sw = demo_swarm()
    a = sw.orchestrators[0].threads[0].agents[0]
    a.activity = "Bash(\x1b[31mred\x1b]0;title\x07)"
    text = render.render(sw, view(), 160, 45).text()
    assert "\x1b" not in text and "\x07" not in text and "Bash([31mred]0;title)" in text


def test_empty_and_unreachable_states():
    from herdr_panorama.model import Swarm
    assert "No Claude Code sessions in herdr" in render.render(Swarm(), View(now=NOW), 160, 45).text()
    assert "herdr is not reachable" in render.render(Swarm(herdr_ok=False), View(now=NOW), 70, 45).text()


def test_long_columns_scroll_to_keep_the_selection_visible():
    sw = demo_swarm()
    o = sw.orchestrators[0]
    from herdr_panorama.model import Thread
    for i in range(12):
        o.threads.insert(0, Thread(id=f"t{i}", name=f"Thread {i}", source="turn", status="active", started=NOW - i))
    v = view(thread="ref:#48", agent="c1d2e3f", focus=1)
    text = render.render(sw, v, 160, 30).text()
    assert "PR #48 retry banner copy" in text and v.offsets[1] > 0


def test_inferred_and_lost_agents_are_marked():
    from herdr_panorama.model import INF, LOST
    sw = demo_swarm()
    t = sw.orchestrators[0].threads[0]
    t.agents[2].status, t.agents[3].status = INF, LOST
    v = view(fin_open={"ref:#42"}, agent=t.agents[2].id, focus=2)
    text = render.render(sw, v, 160, 45).text()
    assert " ✓?" in text and "done?" in text  # pod and status label
    v.agent = t.agents[3].id
    text = render.render(sw, v, 160, 45).text()
    assert " ~ " in text and "lost" in text



# -- design turn 3: lines and animations ----------------------------------------------------

def card_rows(frame, w=160):
    return render.render(demo_swarm(), view(frame=frame), w, 45)


def test_animation_frames_step_the_bar_blink_and_spinner_without_changing_widths():
    from herdr_panorama.render import FAINT, ST
    from herdr_panorama.model import FAIL, RUN
    frames = [card_rows(f) for f in range(8)]
    for scr in frames:
        assert all(len(row) == 160 for row in scr.c)
    # the working bar on Billing API's bottom border (row 6): head at p+3, p = f % (26 + 6) - 3
    for f, scr in enumerate(frames):
        bar = [x for x in range(3, 29) if scr.c[6][x].ch == "━"]
        p = f % 32 - 3
        assert bar == [3 + i for i in range(p, p + 4) if 0 <= i < 26]
        assert all(scr.c[6][x].fg == ST[RUN] for x in bar)  # the whole bar in the running colour
    # attention blinks: 2 frames on, 2 off (the design's floor(F / 2) % 2)
    assert [frames[f].c[4][2].fg for f in range(4)] == [ST[FAIL], ST[FAIL], FAINT, FAINT]
    # a running agent's spinner turns one frame per tick
    assert frames[0].c[9][5].ch != frames[1].c[9][5].ch  # Search refactor's pod
    # nothing else moves: text outside the animated cells is identical
    a, b = frames[0].text().split("\n"), frames[1].text().split("\n")
    assert [i for i, (x, y) in enumerate(zip(a, b)) if x != y] == [6, 9, 11, 16]


def test_quiet_orchestrators_do_not_animate():
    scr0, scr2 = card_rows(0), card_rows(2)
    # Release notes (all done): no edge, no bar, same in every frame
    assert scr0.text().split("\n")[19:22] == scr2.text().split("\n")[19:22]
    assert scr0.c[19][2].ch == "│"
    assert not render.animated(type(demo_swarm())())


# -- review of #4: every card's edge animates (current design), bar colour, joins -----------

def test_r4_thread_and_agent_cards_have_animated_edges():
    from herdr_panorama.render import FAINT, ST
    from herdr_panorama.model import FAIL, REV, WAIT
    f0, f2 = card_rows(0), card_rows(2)
    # thread column x=36: PR #42 (reviewing, steady), PR #44 (failed, blinks), PR #48 (question, blinks)
    assert f0.c[4][36].ch in "┃┨" and f0.c[4][36].fg == ST[REV] and f2.c[4][36].fg == ST[REV]
    assert f0.c[18][36].fg == ST[FAIL] and f2.c[18][36].fg == FAINT
    assert f0.c[25][36].fg == ST[WAIT] and f2.c[25][36].fg == FAINT
    # agent column x=93: the reviewing agent card has an edge; the queued author has none
    rows = f0.text().split("\n")
    rev_row = next(i for i, r in enumerate(rows) if "reviewer a2f81c0" in r)
    q_row = next(i for i, r in enumerate(rows) if "author a1c3d90" in r)
    assert f0.c[rev_row][93].ch in "┃┨" and f0.c[q_row][93].ch in "│┤"


def test_r4_sweeps_are_all_running_colour_and_offset_per_card():
    from herdr_panorama.render import ST
    from herdr_panorama.model import RUN
    v = view(orch="w1:p2", thread="wf:wf_search", agent="j1a2b3c", frame=5)
    scr = render.render(demo_swarm(), v, 160, 45)
    bars = {}
    for y in range(scr.h):
        cells = [x for x in range(scr.w) if scr.c[y][x].ch == "━"]
        if cells:
            assert all(scr.c[y][x].fg == ST[RUN] for x in cells)  # M1: no state-colour head
            bars[y] = cells
    # the design's offsets at frame 5: p = (frame + seed * 7) % (n + 6) - 3
    orch = sorted(c[0] for y, c in bars.items() if c[0] < 30)
    assert set(orch) == {2 + 1 + (5 % 32 - 3)}  # orchestrators: seed 0, all in step (as the design)
    thread = [c for y, c in bars.items() if any(36 <= x < 86 for x in c)]
    assert [x for x in thread[0] if 36 <= x < 86][0] == 36 + 1 + ((5 + 7) % 54 - 3)  # first thread: seed 1
    agent_starts = {min(x for x in c if x >= 93) for y, c in bars.items() if any(x >= 93 for x in c)}
    assert len(agent_starts) == 2  # the two running agents: their own seeds, not in step


def test_r4_links_join_animated_edges():
    scr = card_rows(0)
    rows = scr.text().split("\n")
    assert rows[4][29] == "├"  # Billing API is selected: its edge is on the left, the link leaves right
    assert rows[4][36] == "┨"  # the link enters PR #42's edged border
    assert rows[18][36] == "┨" or rows[18][36] == "┃"  # PR #44: joined when the anchor row meets it
    v = view(orch="w1:p2", thread="wf:wf_search", agent="j1a2b3c", frame=0)
    narrow = render.render(demo_swarm(), v, 70, 45).text().split("\n")
    assert any(r.startswith(" ╰─┨") for r in narrow)  # the narrow tree joins into the edge too
