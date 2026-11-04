//! The wide graph's layout rules on the demo, deep and wide data: Python's
//! tests/test_render.py, tests/test_deep.py, tests/test_wide.py, tests/test_states.py and
//! tests/test_current_card.py cases for the 2a graph (the page goldens are in
//! design_wide.rs, design_states.rs and design_unseen.rs).
//!
//! Since the uniform tree (plan `uniform-agent-tree.md` U4, D5) the columns are depths: the
//! roots' (x 2, 42 wide below 220 columns), then depth 1 at x 51, depth 2 at x 102, … (7
//! apart). The rows and columns asserted are Rust's reviewed renders, no longer Python's.

mod support;

use panorama_core::model::{Agent, Orchestrator, Status, Swarm};
use panorama_core::view::cards::edge::sweep_pos;
use panorama_core::view::nav::nav;
use panorama_core::view::palette::{FAINT, HI, status};
use panorama_core::view::state::{FIN_CARD, NEW_ROW};
use panorama_core::view::{Filter, Marks, Rgb, Screen, Unseen, View, render_graph};
use serde_json::json;
use support::tree_design::{NOW as DEEP_NOW, tree_swarm};
use support::{NO_SESSIONS, fixture, wire_swarm};

fn demo() -> Swarm {
    wire_swarm("inputs/swarm-demo.json", None)
}

fn demo_now() -> f64 {
    fixture("inputs/swarm-demo.json")["now"].as_f64().unwrap()
}

/// test_render.py `view()`: the demo's first root, the reviewer selected below it, the
/// keyboard on the root.
fn demo_view() -> View {
    View {
        orch: Some("w1:p1".into()),
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

/// The first row holding `want`.
fn row_of(s: &Screen, want: &str) -> usize {
    let text = s.text();
    text.lines()
        .position(|r| r.contains(want))
        .unwrap_or_else(|| panic!("missing {want:?}\n{text}"))
}

#[test]
fn the_wide_layout_has_the_depth_columns_and_links() {
    // test_render.py::test_wide_layout_has_the_three_columns_and_links, by depth
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
    // root cards at x=2, width 42; the selected one, the keyboard on it, unfolded
    assert_eq!(seg(&r, 3, 2..44), format!("╭{}╮", "─".repeat(40)));
    assert_eq!(seg(&r, 4, 2..5), "┃  ");
    let row = |y: usize| r[y].iter().collect::<String>();
    assert!(row(4).contains("✗  Claude Code · Billing API") && seg(&r, 4, 0..44).contains("?1"));
    assert!(row(5).contains("failed · working · 4/12"));
    assert!(row(7).contains("g open pane"));
    assert!(row(11).contains("⠸  Claude Code · Search refactor"));
    // depth 1: every agent of the root's threads, the selected one unfolded, live ones as
    // cards, finished ones folded
    for want in [
        "reviewer a2f81c0 · opus",
        "reviewing",
        "now  Bash(Diff totals.py against roun…",
        "g open pane   t transcript   a answer",
        "author a1c3d90 · opus",
        "fixer b3a9c41 · sonnet",
        "author c1d2e3f · sonnet",
        "8 finished · aut",
    ] {
        has(&text, want);
    }
    for gone in ["PR #42 invoice export", "complete", "pr-review"] {
        assert!(!text.contains(gone), "a thread's {gone:?}\n{text}");
    }
    // solid links: the borders become tees, the trunk branches
    assert_eq!(
        [ch(&r, 43, 4), ch(&r, 47, 4), ch(&r, 51, 4)],
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
    // test_render.py::test_question_and_blocked_detail_lines: an agent's question, and a
    // root's own blocking prompt in its unfolded card
    let sw = demo();
    let v = View {
        agent: Some("c1d2e3f".into()),
        focus: 1,
        ..demo_view()
    };
    let text = render(&sw, &v, 160, 45).text();
    has(&text, "? Use \"Retrying…\" or \"Reconnecting…\"");
    has(&text, "needs answer");
    let v = View {
        orch: Some("w1:p3".into()),
        agent: None,
        focus: 0,
        ..demo_view()
    };
    let text = render(&sw, &v, 160, 45).text();
    has(&text, "! Publish the v2 docs now or after");
    has(&text, "release?");
    has(&text, "g open pane  t transcript  a answer"); // the hints fit the card
}

#[test]
fn an_untitled_agent_reads_role_id_and_model() {
    // test_render.py::test_explicit_thread_shows_pct_and_worktree's agent (its thread's
    // phase and worktree went with the thread card)
    let v = View {
        orch: Some("w1:p3".into()),
        agent: Some("d1e2f3a".into()),
        focus: 1,
        ..demo_view()
    };
    has(
        &render(&demo(), &v, 160, 45).text(),
        "writer d1e2f3a · sonnet",
    );
}

#[test]
fn filters() {
    // test_render.py::test_filters, on the root's depth-1 agents
    let sw = demo();
    let v = |filter| View {
        filter,
        ..demo_view()
    };
    assert_eq!(nav(&sw, &v(Filter::Needs))[0], ["w1:p1", "w1:p3", NEW_ROW]);
    assert_eq!(nav(&sw, &v(Filter::Needs))[1], ["b3a9c41", "c1d2e3f"]);
    assert_eq!(
        nav(&sw, &v(Filter::Running))[1],
        ["a1c3d90", "a2f81c0", "b3a9c41", "c1d2e3f"]
    );
    has(&render(&sw, &v(Filter::Needs), 160, 45).text(), "needs me");
}

#[test]
fn long_columns_scroll_to_keep_the_selection_visible() {
    // test_render.py::test_long_columns_scroll_to_keep_the_selection_visible
    let mut sw = demo();
    for i in 0..12 {
        let a: Agent = serde_json::from_value(json!({
            "id": format!("x{i}"), "role": "extra", "status": "run",
            "started": demo_now() + f64::from(i),
        }))
        .unwrap();
        sw.orchestrators[0].threads[0].agents.push(a);
    }
    let v = View {
        agent: Some("x11".into()),
        focus: 1,
        ..demo_view()
    };
    let s = render(&sw, &v, 160, 30);
    has(&s.text(), "extra x11");
    let agents = s.columns.iter().find(|c| c.column == 1).unwrap();
    assert!(agents.offset > 0);
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
        focus: 1,
        agent: Some(inf),
        ..demo_view()
    };
    v.fin_open.insert("w1:p1".into());
    let text = render(&sw, &v, 160, 45).text();
    has(&text, " ✓?"); // pod
    has(&text, "done?"); // status label
    v.agent = Some(lost);
    let text = render(&sw, &v, 160, 45).text();
    has(&text, " ~ ");
    has(&text, "lost");
}

/// The demo at frame `f`, the keyboard in the tree (the root cards folded).
fn frame(f: f64) -> Screen {
    render(
        &demo(),
        &View {
            frame: f,
            focus: 1,
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
        // the working bar on Billing API's bottom border (row 6), seed 0, 40 cells
        let bar: Vec<i32> = (3..43).filter(|x| s.cell(*x, 6).ch == '━').collect();
        let p = sweep_pos(f as f64, 0, 40) as i32;
        let want: Vec<i32> = (p..p + 4)
            .filter(|i| (0..40).contains(i))
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
    // a running root's spinner turns one frame per tick
    assert_ne!(frames[0].cell(5, 9).ch, frames[1].cell(5, 9).ch);
    // nothing else moves: only the rows of the animated cells change
    let (a, b) = (frames[0].text(), frames[1].text());
    let moved: Vec<usize> = a
        .lines()
        .zip(b.lines())
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(moved, MOVED);
}

/// The rows frames 0 and 1 differ in: Billing API's bar, Search refactor's spinner, and the
/// depth-1 cards' bars and spinners (Rust's render, reviewed).
const MOVED: [usize; 4] = [6, 9, 11, 16];

#[test]
fn quiet_roots_do_not_animate() {
    // test_render.py::test_quiet_orchestrators_do_not_animate (Release notes is all done: no
    // edge, no bar, the same in every frame)
    let (f0, f2) = (frame(0.0).text(), frame(2.0).text());
    let lines = |t: &str| {
        t.lines()
            .skip(18)
            .take(4)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert!(lines(&f0)[1].contains("Release notes"));
    assert_eq!(lines(&f0), lines(&f2));
    assert_eq!(frame(0.0).cell(2, 19).ch, '│');
    assert!(!panorama_core::view::moving::animated(
        &Swarm::default(),
        &Marks::new()
    ));
}

#[test]
fn every_card_has_an_animated_edge() {
    // test_render.py::test_r4_thread_and_agent_cards_have_animated_edges, on the depth-1
    // column at x=51: the reviewer (steady), the failed fixer and the asking author (blink),
    // the queued author none
    let (f0, f2) = (frame(0.0), frame(2.0));
    let at = |label: &str| row_of(&f0, label) as i32;
    let rev = status(&Status::Reviewing);
    let y = at("reviewer a2f81c0");
    assert!(
        "┃┨".contains(f0.cell(51, y).ch) && f0.cell(51, y).fg == rev && f2.cell(51, y).fg == rev
    );
    let y = at("fixer b3a9c41");
    assert!(f0.cell(51, y).fg == status(&Status::Failed) && f2.cell(51, y).fg == FAINT);
    let y = at("author c1d2e3f");
    assert!(f0.cell(51, y).fg == status(&Status::Waiting) && f2.cell(51, y).fg == FAINT);
    assert!("│┤".contains(f0.cell(51, at("author a1c3d90")).ch));
}

#[test]
fn sweeps_are_all_running_colour_and_offset_per_card() {
    const FRAME: i32 = 5;
    // test_render.py::test_r4_sweeps_are_all_running_colour_and_offset_per_card
    let v = View {
        orch: Some("w1:p2".into()),
        agent: Some("j1a2b3c".into()),
        focus: 1,
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
    // the roots: seed 0, all in step
    let roots: std::collections::BTreeSet<i32> =
        bars.iter().map(|c| c[0]).filter(|x| *x < 44).collect();
    let p = sweep_pos(f64::from(FRAME), 0, 40) as i32;
    assert_eq!(roots, [3 + p.max(0)].into());
    let agents: std::collections::BTreeSet<i32> = bars
        .iter()
        .filter_map(|c| c.iter().filter(|x| **x >= 51).min().copied())
        .collect();
    assert_eq!(agents.len(), 2); // the two running agents: their own seeds
}

#[test]
fn links_join_animated_edges() {
    // test_render.py::test_r4_links_join_animated_edges (its narrow half is narrow_layout.rs's)
    let f = frame(0.0);
    let r = rows(&f);
    assert_eq!(ch(&r, 43, 4), '├'); // Billing API is selected: the link leaves right
    assert_eq!(ch(&r, 51, row_of(&f, "fixer b3a9c41")), '┨'); // into an edged border
}

fn deep_view(agent: Option<&str>, focus: usize) -> View {
    View {
        orch: Some("w1:p3".into()),
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
    let r = rows(&render(&deep(), &deep_view(Some("l1"), 0), 160, 45));
    let tops: Vec<char> = [2, 43, 51, 94, 102, 145]
        .iter()
        .map(|x| ch(&r, *x, 3))
        .collect();
    assert_eq!(tops, ['╭', '╮', '╭', '╮', '╭', '╮']);
    assert!(seg(&r, 3, 146..160).trim().is_empty());
    let r = rows(&render(&deep(), &deep_view(Some("b1"), 2), 160, 45));
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
    let s = render(&deep(), &deep_view(Some("c1"), 4), 120, 45);
    let text = s.text();
    let r: Vec<&str> = text.lines().collect();
    has(
        r[2],
        "‹ Core Library › lead l1 › researcher r1   3 columns left",
    );
    has(r[2], "Depth 5  1 column right ›"); // c1's children did not fit
    has(&text, "checker c1");
    has(r[4], "researcher r2");
    let last = r[3]
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '╮')
        .map(|(i, _)| i)
        .last();
    assert!(last.is_some_and(|x| x < 118));
    assert_eq!(s.hlo, 3);
}

#[test]
fn a_flat_tree_draws_without_the_scroll_line() {
    // test_deep.py::test_a_flat_thread_draws_like_2a_without_the_scroll_line
    let v = View {
        focus: 1,
        ..demo_view()
    };
    let s = render(&demo(), &v, 160, 45);
    let r = rows(&s);
    assert!(
        r[2].iter().all(|c| *c == ' ')
            && !s.text().lines().nth(44).unwrap().contains("shift+wheel")
    );
    assert_eq!([ch(&r, 43, 4), ch(&r, 51, 4)], ['├', '┤']);
}

#[test]
fn a_sideways_scroll_shows_its_hint_and_fixes_the_first_column() {
    // the key hint test_deep.py's sideways tests read, and View.hcol (Python's sideways scroll)
    let s = render(&deep(), &deep_view(Some("c1"), 4), 160, 45);
    has(
        s.text().lines().nth(44).unwrap(),
        "← back   shift+wheel scroll   enter details",
    );
    let v = View {
        hcol: Some(0),
        ..deep_view(Some("c1"), 4)
    };
    let s = render(&deep(), &v, 160, 45);
    assert_eq!(s.hlo, 0);
    has(
        s.text().lines().nth(2).unwrap(),
        "Depth 3–5  3 columns right ›",
    );
    let v = View {
        hcol: Some(99),
        ..deep_view(Some("c1"), 4)
    };
    assert_eq!(render(&deep(), &v, 160, 45).hlo, 5); // clamped to the last column
}

/// test_wide.py `r1_id`: the depth-2 agent tracing the manifest cache.
fn r1(sw: &Swarm) -> String {
    sw.orchestrators[0]
        .agents()
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
        let text = render(&sw, &deep_view(Some(&sel), 2), w, 45).text();
        assert_eq!(
            text.contains(&format!("general-purpose {sel} · opus 4.5")),
            whole,
            "{w}"
        );
    }
}

#[test]
fn a_roots_background_work_is_in_its_card_not_a_column() {
    // test_states.py::test_the_background_card_is_selectable_in_the_thread_column: the
    // background card went with the thread column; the root's card lists the tasks
    let o: Orchestrator = serde_json::from_value(json!({
        "pane_id": "p", "tab_id": "t", "tab_label": "o", "session_id": "s",
        "bg": [{"id": "b", "kind": "Bash", "description": "watch the build", "started": DEEP_NOW}],
    }))
    .unwrap();
    let sw = Swarm {
        orchestrators: vec![o],
        ..Swarm::default()
    };
    let v = View {
        orch: Some("p".into()),
        now: DEEP_NOW,
        ..View::default()
    };
    assert!(nav(&sw, &v)[1].is_empty());
    has(&render(&sw, &v, 160, 45).text(), "watch the build");
}

/// test_current_card.py: the top border colour of the card titled `label`.
fn top_border(s: &Screen, pane: &str) -> Rgb {
    // Identity, rather than a title substring: a prefixed title may be truncated.
    let hit = s
        .targets
        .iter()
        .find(|hit| hit.target == panorama_core::view::Target::Orch(pane.into()))
        .unwrap();
    s.cell(hit.x0 + 5, hit.y).fg
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
                let (sel, other) = (top_border(&s, &a.pane_id), top_border(&s, &b.pane_id));
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
    assert_eq!(top_border(&render(&sw, &v, 160, 45), &a.pane_id), HI);
}

#[test]
fn scrolled_off_roots_link_their_tree_in_from_the_left() {
    // the focus on depth 2 of the deep tree: the roots are off to the left, the depth-1
    // cards' link comes in from the screen's edge
    let s = render(&deep(), &deep_view(Some("r1"), 2), 160, 45);
    let text = s.text();
    let r: Vec<&str> = text.lines().collect();
    let left: Vec<String> = (2..6).map(|y| r[y].chars().take(8).collect()).collect();
    assert_eq!(left, ["  ‹ Core", "    ╭───", "─┬──┨  ⠇", " │  ╰───"]);
    assert!(
        r[2].starts_with("  ‹ Core Library   1 column left"),
        "{}",
        r[2]
    );
    assert!(r[2].ends_with("Depth 4–5  2 columns right ›"), "{}", r[2]);
    assert_eq!(s.hlo, 1);
}

#[test]
fn a_tree_taller_than_the_screen_says_more_below() {
    let s = render(&deep(), &deep_view(Some("v1"), 1), 160, 14);
    let want = format!("{}↓ more below", " ".repeat(53));
    assert_eq!(s.text().lines().nth(10).unwrap(), want);
}

#[test]
fn a_selected_agent_in_a_column_scrolled_off_does_not_unfold() {
    // r1 (depth 2) is selected but its column is scrolled off to the left: it keeps a folded
    // card's height, so the cards below it stay where they are with nothing selected there
    let rows_of = |agent: Option<&str>| {
        let v = View {
            hcol: Some(3),
            ..deep_view(agent, 2)
        };
        let s = render(&deep(), &v, 160, 45);
        assert_eq!(s.hlo, 3);
        let text = s.text();
        ["checker c1", "shard s1", "shard s8", "3 finished"]
            .map(|w| text.lines().position(|r| r.contains(w)))
    };
    let selected = rows_of(Some("r1"));
    assert!(selected.iter().all(Option::is_some), "{selected:?}");
    assert_eq!(selected, rows_of(Some("v1")));
}

#[test]
fn more_below_shows_only_when_the_tree_is_taller_than_the_column() {
    // the boundary: one row taller than the column, then exactly fitting
    let more = |h| {
        render(&deep(), &deep_view(Some("l1"), 1), 160, h)
            .text()
            .contains("more below")
    };
    assert!(more(MORE_H));
    assert!(!more(MORE_H + 1));
}

/// The deep tree's height with l1 unfolded, as the screen height it fits in, less one.
const MORE_H: i32 = 42;

#[test]
fn the_path_above_the_columns_cuts_each_name_at_28() {
    // a 29-character tab label: 27 characters and "…"
    let mut sw = deep();
    sw.orchestrators[0].tab_label = "Core Library of plugins 12345".into();
    let s = render(&sw, &deep_view(Some("c1"), 4), 160, 45);
    let want = "  ‹ Core Library of plugins 123… › lead l1 › researcher r1   3 columns left";
    assert!(
        s.text().lines().nth(2).unwrap().starts_with(want),
        "{}",
        s.text()
    );
}

#[test]
fn a_selected_agent_scrolled_off_keeps_a_folded_height_for_the_cards_below_it() {
    // a spawned x (selected) and y, y spawned z; only z's column is shown: x keeps three
    // rows, so y and z start right below it
    let o: Orchestrator = serde_json::from_value(json!({
        "pane_id": "p", "tab_id": "tt", "tab_label": "o", "session_id": null,
        "threads": [{"id": "agents", "name": "n", "source": "session", "agents": [
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
        agent: Some("x".into()),
        focus: 2,
        hcol: Some(3),
        now: 10.0,
        ..View::default()
    };
    let s = render(&sw, &v, 160, 45);
    assert_eq!(s.hlo, 3);
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

#[test]
fn the_roots_own_finished_group_folds_by_its_pane() {
    let v = View {
        agent: Some(format!("{FIN_CARD}w1:p1")),
        focus: 1,
        ..demo_view()
    };
    let mut open = v.clone();
    open.fin_open.insert("w1:p1".into());
    let (folded, unfolded) = (
        render(&demo(), &v, 160, 45).text(),
        render(&demo(), &open, 160, 45).text(),
    );
    assert!(
        folded.contains("8 finished") && folded.contains('▸'),
        "{folded}"
    );
    assert!(
        unfolded.contains('▾') && unfolded.contains("author e1a2b3c"),
        "{unfolded}"
    );
}
