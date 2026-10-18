//! The wide graph's layout rules on the demo, deep and wide data: Python's
//! tests/test_render.py, tests/test_deep.py, tests/test_wide.py, tests/test_states.py and
//! tests/test_current_card.py cases for the 2a graph (the design goldens are in
//! design_wide.rs, design_states.rs and design_unseen.rs).

mod support;

use panorama_core::model::{Orchestrator, Status, Swarm, Thread};
use panorama_core::view::nav::nav;
use panorama_core::view::palette::{FAINT, HI, status};
use panorama_core::view::state::{BG_CARD, NEW_ROW};
use panorama_core::view::{Filter, Marks, Rgb, Screen, Unseen, View, render_graph};
use serde_json::json;
use support::tree_design::{NOW as DEEP_NOW, TID, tree_swarm};
use support::{NO_SESSIONS, fixture, wire_swarm};

fn demo() -> Swarm {
    wire_swarm("inputs/swarm-demo.json", None)
}

fn demo_now() -> f64 {
    fixture("inputs/swarm-demo.json")["now"].as_f64().unwrap()
}

/// test_render.py `view()`: the demo's first orchestrator, thread and agent.
fn demo_view() -> View {
    View {
        orch: Some("w1:p1".into()),
        thread: Some("ref:#42".into()),
        agent: Some("a2f81c0".into()),
        now: demo_now(),
        ..View::default()
    }
}

fn render(sw: &Swarm, v: &View, w: i32, h: i32) -> Screen {
    render_graph(sw, v, w, h, None, NO_SESSIONS)
}

/// The rows as characters (Python indexes a row by character).
fn rows(s: &Screen) -> Vec<Vec<char>> {
    s.text().lines().map(|l| l.chars().collect()).collect()
}

fn ch(rows: &[Vec<char>], x: usize, y: usize) -> char {
    rows[y].get(x).copied().unwrap_or(' ')
}

fn seg(rows: &[Vec<char>], y: usize, x: std::ops::Range<usize>) -> String {
    x.map(|x| ch(rows, x, y)).collect()
}

fn has(text: &str, want: &str) {
    assert!(text.contains(want), "missing {want:?}\n{text}");
}

#[test]
fn the_wide_layout_has_the_three_columns_and_links() {
    // test_render.py::test_wide_layout_has_the_three_columns_and_links
    let s = render(&demo(), &demo_view(), 160, 45);
    let text = s.text();
    let r = rows(&s);
    assert!(r.len() == 45 && r.iter().all(|r| r.len() <= 160));
    assert!(text.starts_with(" herdr panorama    all   running   needs me"));
    assert!(
        text.lines()
            .next()
            .unwrap()
            .ends_with("? 1  ! 1  ✗ 1    10:04")
    );
    // orchestrator cards at x=2, width 28
    assert_eq!(seg(&r, 3, 2..30), format!("╭{}╮", "─".repeat(26)));
    assert_eq!(seg(&r, 4, 2..5), "┃  ");
    let row = |y: usize| r[y].iter().collect::<String>();
    assert!(row(4).contains("✗  Billing API") && seg(&r, 4, 0..31).contains("?1"));
    assert!(row(5).contains("failed · working"));
    assert!(seg(&r, 6, 2..30).ends_with("─ pr-review ─╯"));
    assert!(row(14).contains("!  Docs site") && row(15).contains("blocked"));
    // the selected thread unfolds into its steps with times
    assert!(row(4).contains("» PR #42 invoice export"));
    assert!(row(5).contains("review r2 · 21m"));
    for want in [
        "✓ author       09:02–09:20",
        "» review r2    since 09:43 · 21m   0B 1H 2M",
        "· CI",
        "· merge",
        // other active threads collapse; done threads fold into one card
        "✗ PR #44 health badge",
        "? PR #48 retry banner copy",
        "✓ 1 complete  PR #40 currency codes",
        // the agents: the selected one opened, live ones as cards, finished ones folded
        "reviewer a2f81c0 · opus",
        "reviewing",
        "now  Bash(Diff totals.py against roun…",
        "g open pane   t transcript   a answer",
        "author a1c3d90 · opus",
        "3 finished · explorer ×3",
    ] {
        has(&text, want);
    }
    // solid links: the borders become tees, the trunk branches
    assert_eq!(
        [ch(&r, 29, 4), ch(&r, 32, 4), ch(&r, 36, 4)],
        ['├', '┬', '┨']
    );
    assert_eq!(
        [ch(&r, 85, 4), ch(&r, 89, 4), ch(&r, 93, 4)],
        ['├', '┬', '┤']
    );
    // legend and keys
    assert!(
        row(42).contains("✓  done   ✓? done?   ⠋  running   »  review")
            && row(42).contains("~  lost")
    );
    assert!(row(44).starts_with(
        " ↑↓ move   → open   ← back   enter details   g go to pane   a answer   i inbox"
    ));
}

#[test]
fn question_and_blocked_detail_lines() {
    // test_render.py::test_question_and_blocked_detail_lines
    let sw = demo();
    let v = View {
        thread: Some("ref:#48".into()),
        agent: Some("c1d2e3f".into()),
        ..demo_view()
    };
    let text = render(&sw, &v, 160, 45).text();
    has(&text, "? Use \"Retrying…\" or \"Reconnecting…\"");
    has(&text, "  for the banner? The design doc and");
    has(&text, "needs answer");
    let v = View {
        orch: Some("w1:p3".into()),
        thread: Some("main:w1:p3".into()),
        agent: Some("main:w1:p3".into()),
        ..demo_view()
    };
    let text = render(&sw, &v, 160, 45).text();
    has(&text, "! Publish the v2 docs now or after the");
    has(&text, "  release?");
    has(&text, "g open pane  t transcript  a answer    │"); // the hints fit the card
}

#[test]
fn an_explicit_thread_shows_its_pct_and_worktree() {
    // test_render.py::test_explicit_thread_shows_pct_and_worktree
    let v = View {
        orch: Some("w1:p3".into()),
        thread: Some("x:docs-v2".into()),
        agent: Some("d1e2f3a".into()),
        ..demo_view()
    };
    let text = render(&demo(), &v, 160, 45).text();
    has(&text, "write · 33m · 60%");
    has(&text, "wt/docs-v2");
    has(&text, "writer d1e2f3a · sonnet");
}

#[test]
fn filters() {
    // test_render.py::test_filters
    let sw = demo();
    let v = |filter| View {
        filter,
        ..demo_view()
    };
    assert_eq!(nav(&sw, &v(Filter::Needs))[0], ["w1:p1", "w1:p3", NEW_ROW]);
    assert_eq!(nav(&sw, &v(Filter::Needs))[1], ["ref:#44", "ref:#48"]);
    assert_eq!(
        nav(&sw, &v(Filter::Running))[1],
        ["ref:#42", "ref:#44", "ref:#48"]
    );
    has(&render(&sw, &v(Filter::Needs), 160, 45).text(), "needs me");
}

#[test]
fn long_columns_scroll_to_keep_the_selection_visible() {
    // test_render.py::test_long_columns_scroll_to_keep_the_selection_visible
    let mut sw = demo();
    for i in 0..12 {
        let t: Thread = serde_json::from_value(json!({
            "id": format!("t{i}"), "name": format!("Thread {i}"), "source": "turn",
            "status": "active", "started": demo_now() - f64::from(i),
        }))
        .unwrap();
        sw.orchestrators[0].threads.insert(0, t);
    }
    let v = View {
        thread: Some("ref:#48".into()),
        agent: Some("c1d2e3f".into()),
        focus: 1,
        ..demo_view()
    };
    let s = render(&sw, &v, 160, 30);
    has(&s.text(), "PR #48 retry banner copy");
    let threads = s.columns.iter().find(|c| c.column == 1).unwrap();
    assert!(threads.offset > 0);
}

#[test]
fn inferred_and_lost_agents_are_marked() {
    // test_render.py::test_inferred_and_lost_agents_are_marked
    let mut sw = demo();
    let t = &mut sw.orchestrators[0].threads[0];
    t.agents[2].status = Status::Inferred;
    t.agents[3].status = Status::Lost;
    let (inf, lost) = (t.agents[2].id.clone(), t.agents[3].id.clone());
    let mut v = View {
        focus: 2,
        agent: Some(inf),
        ..demo_view()
    };
    v.fin_open.insert("ref:#42".into());
    let text = render(&sw, &v, 160, 45).text();
    has(&text, " ✓?"); // pod
    has(&text, "done?"); // status label
    v.agent = Some(lost);
    let text = render(&sw, &v, 160, 45).text();
    has(&text, " ~ ");
    has(&text, "lost");
}

fn frame(f: f64) -> Screen {
    render(
        &demo(),
        &View {
            frame: f,
            ..demo_view()
        },
        160,
        45,
    )
}

#[test]
fn animation_frames_step_the_bar_blink_and_spinner_without_changing_widths() {
    // test_render.py::test_animation_frames_step_the_bar_blink_and_spinner_without_changing_widths
    let frames: Vec<Screen> = (0..8).map(|f| frame(f64::from(f))).collect();
    for (f, s) in frames.iter().enumerate() {
        assert_eq!(s.w(), 160);
        // the working bar on Billing API's bottom border (row 6): p = f % (26 + 6) - 3
        let bar: Vec<i32> = (3..29).filter(|x| s.cell(*x, 6).ch == '━').collect();
        let p = f as i32 % 32 - 3;
        let want: Vec<i32> = (p..p + 4)
            .filter(|i| (0..26).contains(i))
            .map(|i| 3 + i)
            .collect();
        assert_eq!(bar, want, "frame {f}");
        assert!(
            bar.iter()
                .all(|x| s.cell(*x, 6).fg == status(&Status::Running))
        );
    }
    // attention blinks: 2 frames on, 2 off (the design's floor(F / 2) % 2)
    let fail = status(&Status::Failed);
    let blink: Vec<Rgb> = (0..4).map(|f| frames[f].cell(2, 4).fg).collect();
    assert_eq!(blink, [fail, fail, FAINT, FAINT]);
    // a running agent's spinner turns one frame per tick
    assert_ne!(frames[0].cell(5, 9).ch, frames[1].cell(5, 9).ch);
    // nothing else moves: text outside the animated cells is identical
    let (a, b) = (frames[0].text(), frames[1].text());
    let moved: Vec<usize> = a
        .lines()
        .zip(b.lines())
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(moved, [6, 9, 11, 16]);
}

#[test]
fn quiet_orchestrators_do_not_animate() {
    // test_render.py::test_quiet_orchestrators_do_not_animate (Release notes is all done: no
    // edge, no bar, the same in every frame)
    let (f0, f2) = (frame(0.0).text(), frame(2.0).text());
    let lines = |t: &str| {
        t.lines()
            .skip(19)
            .take(3)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(&f0), lines(&f2));
    assert_eq!(frame(0.0).cell(2, 19).ch, '│');
    assert!(!panorama_core::view::moving::animated(
        &Swarm::default(),
        &Marks::new()
    ));
}

#[test]
fn every_card_has_an_animated_edge() {
    // test_render.py::test_r4_thread_and_agent_cards_have_animated_edges: the thread column
    // at x=36: PR #42 (reviewing, steady), PR #44 (failed, blinks), PR #48 (question, blinks)
    let (f0, f2) = (frame(0.0), frame(2.0));
    let rev = status(&Status::Reviewing);
    assert!(
        "┃┨".contains(f0.cell(36, 4).ch) && f0.cell(36, 4).fg == rev && f2.cell(36, 4).fg == rev
    );
    assert!(f0.cell(36, 18).fg == status(&Status::Failed) && f2.cell(36, 18).fg == FAINT);
    assert!(f0.cell(36, 25).fg == status(&Status::Waiting) && f2.cell(36, 25).fg == FAINT);
    // the agent column at x=93: the reviewing agent's card has an edge; the queued author none
    let text = f0.text();
    let row_of = |w: &str| text.lines().position(|r| r.contains(w)).unwrap() as i32;
    assert!("┃┨".contains(f0.cell(93, row_of("reviewer a2f81c0")).ch));
    assert!("│┤".contains(f0.cell(93, row_of("author a1c3d90")).ch));
}

#[test]
fn sweeps_are_all_running_colour_and_offset_per_card() {
    const FRAME: i32 = 5;
    // test_render.py::test_r4_sweeps_are_all_running_colour_and_offset_per_card
    let v = View {
        orch: Some("w1:p2".into()),
        thread: Some("wf:wf_search".into()),
        agent: Some("j1a2b3c".into()),
        frame: f64::from(FRAME),
        ..demo_view()
    };
    let s = render(&demo(), &v, 160, 45);
    let run = status(&Status::Running);
    let mut bars: Vec<Vec<i32>> = vec![];
    for y in 0..s.h() {
        let cells: Vec<i32> = (0..s.w()).filter(|x| s.cell(*x, y).ch == '━').collect();
        assert!(cells.iter().all(|x| s.cell(*x, y).fg == run)); // no state-colour head
        if !cells.is_empty() {
            bars.push(cells);
        }
    }
    // the design's offsets at frame 5: p = (frame + seed * 7) % lap * (n + 6) / lap - 3
    let orch: std::collections::BTreeSet<i32> =
        bars.iter().map(|c| c[0]).filter(|x| *x < 30).collect();
    assert_eq!(orch, [2 + 1 + (FRAME % 32 - 3)].into()); // seed 0, all in step
    // the first thread: seed 1; a 50-column card laps in at most 48 frames
    let thread = bars
        .iter()
        .find(|c| c.iter().any(|x| (36..86).contains(x)))
        .unwrap();
    let first = thread.iter().find(|x| (36..86).contains(*x)).unwrap();
    assert_eq!(*first, 36 + 1 + ((FRAME + 7) % 48 * 54 / 48 - 3));
    let agents: std::collections::BTreeSet<i32> = bars
        .iter()
        .filter_map(|c| c.iter().filter(|x| **x >= 93).min().copied())
        .collect();
    assert_eq!(agents.len(), 2); // the two running agents: their own seeds
}

#[test]
fn links_join_animated_edges() {
    // test_render.py::test_r4_links_join_animated_edges (its narrow half is narrow_layout.rs's)
    let r = rows(&frame(0.0));
    assert_eq!(ch(&r, 29, 4), '├'); // Billing API is selected: the link leaves right
    assert_eq!(ch(&r, 36, 4), '┨'); // into PR #42's edged border
    assert!("┨┃".contains(ch(&r, 36, 18)));
}

fn deep_view(agent: Option<&str>, focus: usize) -> View {
    View {
        orch: Some("w1:p3".into()),
        thread: Some(TID.into()),
        agent: agent.map(Into::into),
        focus,
        now: DEEP_NOW,
        ..View::default()
    }
}

fn deep() -> Swarm {
    wire_swarm("inputs/swarm-deep.json", None)
}

#[test]
fn columns_keep_their_width_and_align_left() {
    // test_deep.py::test_columns_keep_their_width_and_align_left: a column never stretches
    // into the space left on the right (a stretched card's sweep ran too long)
    let r = rows(&render(&deep(), &deep_view(None, 1), 160, 45));
    assert!(
        ch(&r, 93, 4) == '┨' && ch(&r, 136, 4) == '│' && seg(&r, 4, 137..160).trim().is_empty()
    );
    let r = rows(&render(&deep(), &deep_view(Some("b1"), 3), 160, 45));
    let tops: Vec<char> = [4, 47, 55, 98, 106, 149]
        .iter()
        .map(|x| ch(&r, *x, 3))
        .collect();
    assert_eq!(tops, ['╭', '╮', '╭', '╮', '╭', '╮']);
    assert!(seg(&r, 3, 150..160).trim().is_empty());
}

#[test]
fn a_narrow_screen_shrinks_the_last_column_and_keeps_the_focus_and_its_parents() {
    // test_deep.py::test_narrow_screens_shrink_the_last_column_and_keep_the_focus_and_its_parents
    let s = render(&deep(), &deep_view(Some("c1"), 5), 120, 45);
    let text = s.text();
    let r: Vec<&str> = text.lines().collect();
    has(
        r[2],
        "‹ Core Library › Refactor: plugin loader › lead l1 › researcher r1   4 columns left",
    );
    has(r[2], "depth 5  1 column right ›"); // c1's children did not fit
    has(&text, "checker c1");
    has(r[4], "researcher r2");
    let last = r[3]
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '╮')
        .map(|(i, _)| i)
        .last();
    assert!(last.is_some_and(|x| x < 118));
    assert_eq!(s.hlo, 4);
}

#[test]
fn a_flat_thread_draws_like_2a_without_the_scroll_line() {
    // test_deep.py::test_a_flat_thread_draws_like_2a_without_the_scroll_line
    let v = View {
        orch: Some("w1:p1".into()),
        thread: Some("ref:#42".into()),
        agent: Some("a2f81c0".into()),
        focus: 2,
        ..demo_view()
    };
    let s = render(&demo(), &v, 160, 45);
    let r = rows(&s);
    assert!(
        r[2].iter().all(|c| *c == ' ')
            && !s.text().lines().nth(44).unwrap().contains("shift+wheel")
    );
    assert_eq!([ch(&r, 85, 4), ch(&r, 93, 4)], ['├', '┤']);
}

#[test]
fn a_sideways_scroll_shows_its_hint_and_fixes_the_first_column() {
    // the key hint test_deep.py's sideways tests read, and View.hcol (Python's sideways scroll)
    let s = render(&deep(), &deep_view(Some("c1"), 5), 160, 45);
    has(
        s.text().lines().nth(44).unwrap(),
        "← back   shift+wheel scroll   enter details",
    );
    let v = View {
        hcol: Some(0),
        ..deep_view(Some("c1"), 5)
    };
    let s = render(&deep(), &v, 160, 45);
    assert_eq!(s.hlo, 0);
    has(
        s.text().lines().nth(2).unwrap(),
        "depth 2–5  4 columns right ›",
    );
    let v = View {
        hcol: Some(99),
        ..deep_view(Some("c1"), 5)
    };
    assert_eq!(render(&deep(), &v, 160, 45).hlo, 6); // clamped to the last column
}

/// test_wide.py `r1_id`: the depth-2 agent tracing the manifest cache.
fn r1(sw: &Swarm) -> String {
    let t = &sw.orchestrators[0].threads[0];
    t.agents
        .iter()
        .find(|a| a.depth == 2 && a.activity.starts_with("tracing"))
        .unwrap()
        .id
        .clone()
}

#[test]
fn the_full_title_fits_from_240_columns() {
    // test_wide.py::test_the_full_title_fits_from_240_columns
    let sw = tree_swarm(&fixture("design_7.json"));
    let sel = r1(&sw);
    for (w, whole) in [(160, false), (240, true), (320, true)] {
        let text = render(&sw, &deep_view(Some(&sel), 3), w, 45).text();
        assert_eq!(
            text.contains(&format!("general-purpose {sel} · opus 4.5")),
            whole,
            "{w}"
        );
    }
}

#[test]
fn the_background_card_is_selectable_in_the_thread_column() {
    // test_states.py::test_the_background_card_is_selectable_in_the_thread_column
    let o: Orchestrator = serde_json::from_value(json!({
        "pane_id": "p", "tab_id": "t", "tab_label": "o", "session_id": "s",
        "bg": [{"id": "b", "kind": "Bash", "description": "w", "started": DEEP_NOW}],
    }))
    .unwrap();
    let sw = Swarm {
        orchestrators: vec![o],
        ..Swarm::default()
    };
    let v = View {
        orch: Some("p".into()),
        ..View::default()
    };
    assert_eq!(nav(&sw, &v)[1][0], format!("{BG_CARD}p"));
}

/// test_current_card.py: the top border colour of the card titled `label`.
fn top_border(s: &Screen, label: &str) -> Rgb {
    let text = s.text();
    let y = text
        .lines()
        .position(|r| r.chars().take(60).collect::<String>().contains(label))
        .unwrap()
        - 1;
    let row: Vec<char> = text.lines().nth(y).unwrap().chars().collect();
    let x = row.iter().position(|c| *c == '╭').unwrap();
    s.cell(x as i32 + 5, y as i32).fg
}

fn lum(c: Rgb) -> f64 {
    0.2126 * f64::from(c.0) + 0.7152 * f64::from(c.1) + 0.0722 * f64::from(c.2)
}

fn dist(a: Rgb, b: Rgb) -> f64 {
    let d = |x: u8, y: u8| (f64::from(x) - f64::from(y)).powi(2);
    (d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)).sqrt()
}

#[test]
fn the_selected_card_stays_apart_through_a_whole_pulse() {
    // test_current_card.py::test_the_selected_card_stays_apart_through_a_whole_pulse: a failed
    // or question pulse reaches the full event colour at its peak; the selected card must
    // still differ there (focused: brighter by a margin; selected elsewhere: another colour)
    let sw = demo();
    let (a, b) = (&sw.orchestrators[0], &sw.orchestrators[1]);
    for event in [Unseen::Fail, Unseen::Wait, Unseen::Done] {
        let unseen: Marks = [a, b]
            .iter()
            .map(|o| (o.agents().next().unwrap().id.clone(), event))
            .collect();
        for focused in [true, false] {
            let (mut worst_lum, mut worst_dist) = (f64::MAX, f64::MAX);
            for f in 0..=120 {
                let v = View {
                    orch: Some(a.pane_id.clone()),
                    focus: if focused { 0 } else { 1 },
                    frame: f64::from(f) / 10.0,
                    unseen: unseen.clone(),
                    now: demo_now(),
                    ..View::default()
                };
                let s = render(&sw, &v, 160, 45);
                let (sel, other) = (top_border(&s, &a.tab_label), top_border(&s, &b.tab_label));
                worst_lum = worst_lum.min(lum(sel) - lum(other));
                worst_dist = worst_dist.min(dist(sel, other));
            }
            if focused {
                assert!(
                    worst_lum >= 15.0 && worst_dist >= 50.0,
                    "{event:?}: {worst_lum} {worst_dist}"
                );
            } else {
                assert!(worst_dist >= 40.0, "{event:?}: {worst_dist}");
            }
        }
    }
}

#[test]
fn without_a_pulse_the_selection_is_unchanged() {
    // test_current_card.py::test_without_a_pulse_the_selection_is_unchanged
    let sw = demo();
    let a = &sw.orchestrators[0];
    let v = View {
        orch: Some(a.pane_id.clone()),
        now: demo_now(),
        ..View::default()
    };
    assert_eq!(top_border(&render(&sw, &v, 160, 45), &a.tab_label), HI);
}

#[test]
fn scrolled_off_orchestrators_link_their_threads_in_from_the_left() {
    // the focus on depth 1 of the deep thread: the orchestrators are off to the left, the
    // thread card's link comes in from the screen's edge (captured from Python's render)
    let s = render(&deep(), &deep_view(Some("l1"), 2), 160, 45);
    let text = s.text();
    let r: Vec<&str> = text.lines().collect();
    let left: Vec<String> = (2..9).map(|y| r[y].chars().take(8).collect()).collect();
    assert_eq!(
        left,
        [
            "  ‹ Core",
            "    ╭───",
            "────┨ ✗ ",
            "    ┃   ",
            "    ┃   ",
            "    ┃   ",
            "    ┃   "
        ]
    );
    let line = "  ‹ Core Library   1 column left                                                                                                  depth 3–5  3 columns right ›";
    assert_eq!(r[2], line);
    assert_eq!(s.hlo, 1);
}

#[test]
fn a_tree_taller_than_the_screen_says_more_below() {
    // captured from Python's render at 14 rows
    let s = render(&deep(), &deep_view(Some("v1"), 2), 160, 14);
    let want = format!("{}↓ more below", " ".repeat(63));
    assert_eq!(s.text().lines().nth(10).unwrap(), want);
}

#[test]
fn a_selected_agent_in_a_column_scrolled_off_does_not_unfold() {
    // r1 (depth 2) is selected but its column is scrolled off to the left: it keeps a folded
    // card's height, so b1's children below it stay where Python drew them
    let v = View {
        hcol: Some(4),
        ..deep_view(Some("r1"), 3)
    };
    let s = render(&deep(), &v, 160, 45);
    assert_eq!(s.hlo, 4);
    let text = s.text();
    let row = |w: &str| text.lines().position(|r| r.contains(w));
    let rows: Vec<Option<usize>> = ["checker c1", "shard s1", "shard s8", "3 finished"]
        .map(row)
        .into();
    assert_eq!(rows, [Some(4), Some(16), Some(32), Some(36)]); // captured from Python's render
}

#[test]
fn more_below_shows_only_when_the_tree_is_taller_than_the_column() {
    // the boundary, from Python's render: at 42 rows the tree is one row taller than the
    // column; at 43 it fits exactly
    let more = |h| {
        render(&deep(), &deep_view(Some("l1"), 2), 160, h)
            .text()
            .contains("more below")
    };
    assert!(more(42));
    assert!(!more(43));
}

#[test]
fn the_path_above_the_columns_cuts_each_name_at_28() {
    // a 29-character tab label: 27 characters and "…" (captured from Python's render)
    let mut sw = deep();
    sw.orchestrators[0].tab_label = "Core Library of plugins 12345".into();
    let s = render(&sw, &deep_view(Some("c1"), 5), 160, 45);
    let want = "  ‹ Core Library of plugins 123… › Refactor: plugin loader › lead l1 › researcher r1   4 columns left";
    assert!(s.text().lines().nth(2).unwrap().starts_with(want));
}

#[test]
fn a_selected_agent_scrolled_off_keeps_a_folded_height_for_the_cards_below_it() {
    // a spawned x (selected) and y, y spawned z; only z's column is shown: x keeps three
    // rows, so y and z start right below it (z's row captured from Python's render)
    let o: Orchestrator = serde_json::from_value(json!({
        "pane_id": "p", "tab_id": "tt", "tab_label": "o", "session_id": null,
        "threads": [{"id": "t", "name": "n", "source": "turn", "status": "active", "agents": [
            {"id": "a", "status": "run", "started": 1.0},
            {"id": "x", "parent": "a", "status": "run", "started": 2.0, "activity": "long ".repeat(20)},
            {"id": "y", "parent": "a", "status": "run", "started": 3.0},
            {"id": "z", "parent": "y", "status": "run", "started": 4.0},
        ]}],
    }))
    .unwrap();
    let sw = Swarm {
        orchestrators: vec![o],
        ..Swarm::default()
    };
    let v = View {
        orch: Some("p".into()),
        thread: Some("t".into()),
        agent: Some("x".into()),
        focus: 3,
        hcol: Some(4),
        now: 10.0,
        ..View::default()
    };
    let s = render(&sw, &v, 160, 45);
    assert_eq!(s.hlo, 4);
    let text = s.text();
    let z = text.lines().position(|r| r.contains("agent z"));
    assert_eq!(z, Some(8));
    assert!(
        text.lines()
            .nth(8)
            .unwrap()
            .starts_with("────┨  ⠋  agent z   running")
    );
}
