//! The wide graph's columns, each drawn on its own, against the cells the design's renders
//! and the app-screen goldens show for them (`design_8.json` turn 8's 2a views,
//! `screens/demo-wide.json`, `screens/deep.json`). The links between the columns are drawn
//! by the page (P4 PR 4b): where a link joins a border, the golden's tee is compared as the
//! plain border.

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::graph::agent_card::{AgentCtx, agent_card};
use panorama_core::view::graph::column::{Column, Place};
use panorama_core::view::graph::orchs::orch_column;
use panorama_core::view::graph::threads::thread_column;
use panorama_core::view::nav::{default_children, find_agent, find_orch, find_thread};
use panorama_core::view::state::BG_CARD;
use panorama_core::view::{Filter, Screen, View};
use serde_json::Value;
use std::ops::Range;
use support::states::{NOW, states_swarm};
use support::{Shown, assert_same, cells, fixture, golden, wire_swarm};

/// The golden's cell with a link's tee on a card border read as the border.
fn unjoined(c: &Shown) -> Shown {
    let ch = match c.ch {
        '├' | '┤' => '│',
        '┨' | '┠' => '┃',
        ch => ch,
    };
    Shown { ch, ..c.clone() }
}

/// Compares `canvas` (drawn at `(x0, top)` on the page, not scrolled) with `want` over the
/// page cells `xs` × `ys`.
fn assert_at(
    name: &str,
    want: &[Vec<Shown>],
    canvas: &Screen,
    (x0, top): (usize, usize),
    xs: Range<usize>,
    ys: &[usize],
) {
    let got = cells(canvas);
    let mut d = vec![];
    for &y in ys {
        for x in xs.clone() {
            let w = unjoined(&want[y][x]);
            let g = &got[y - top][x - x0];
            if &w != g {
                d.push(format!("({x},{y}) want {w:?} got {g:?}"));
            }
        }
    }
    assert!(
        d.is_empty(),
        "{name}: {} cells differ, first:\n{}",
        d.len(),
        d[..d.len().min(5)].join("\n")
    );
}

// -- turn 8 (tests/test_states.py's 2a cases) --------------------------------------------

/// test_states.py `graph`'s view: the keyboard in the agents' column.
fn states_view(frame: f64, sel: (&str, Option<&str>, Option<&str>), filter: Filter) -> View {
    View {
        orch: Some(sel.0.into()),
        thread: sel.1.map(Into::into),
        agent: sel.2.map(Into::into),
        focus: 2,
        frame,
        now: NOW,
        filter,
        ..View::default()
    }
}

/// The orchestrator cards: tabs 5-8 (rows 23-42) whole; the earlier four keep what the app
/// drew before turn 8, so only their state lines are compared (test_states.py `ORCH_Y`).
fn orch_rows(last: usize) -> Vec<usize> {
    [5, 10, 15, 20].into_iter().chain(23..last).collect()
}

fn turn_8() -> (Swarm, Vec<(String, Value)>) {
    let eight = fixture("design_8.json");
    let frames = eight["frames"]
        .as_object()
        .unwrap()
        .clone()
        .into_iter()
        .collect();
    (states_swarm(&eight["data"]), frames)
}

#[test]
fn turn_8_orchestrator_column_and_background_card() {
    let (swarm, frames) = turn_8();
    let bg = format!("{BG_CARD}w1:p5");
    for (key, panes) in frames {
        let view = states_view(
            key.parse().unwrap(),
            ("w1:p5", Some(&bg), None),
            Filter::All,
        );
        let want = golden(&panes["A"]);
        let orchs = orch_column(&swarm, &view, 28);
        assert_at(
            &format!("A orchestrators {key}"),
            &want,
            &orchs.canvas,
            (2, 3),
            2..30,
            &orch_rows(43),
        );
        let o = find_orch(&swarm, Some("w1:p5")).unwrap();
        let threads = thread_column(o, &view, 50);
        assert_at(
            &format!("A background {key}"),
            &want,
            &threads.canvas,
            (36, 3),
            36..86,
            &[3, 4, 5, 6, 7],
        );
    }
}

#[test]
fn turn_8_waiting_agent_card() {
    let (swarm, frames) = turn_8();
    for (key, panes) in frames {
        let view = states_view(
            key.parse().unwrap(),
            ("w1:p4", Some("p214"), Some("n1")),
            Filter::All,
        );
        let want = golden(&panes["B"]);
        let orchs = orch_column(&swarm, &view, 28);
        assert_at(
            &format!("B orchestrators {key}"),
            &want,
            &orchs.canvas,
            (2, 3),
            2..30,
            &orch_rows(43),
        );
        let t = find_thread(find_orch(&swarm, Some("w1:p4")), Some("p214")).unwrap();
        let n1 = find_agent(Some(t), Some("n1")).unwrap();
        let mut cv = Screen::new(44, 20);
        let place = Place {
            x: 0,
            y: 0,
            w: 44,
            is_sel: true,
            focus: true,
        };
        let ctx = AgentCtx {
            t,
            inbox_ok: swarm.inbox_ok,
            on_path: false,
            kids_off: 0,
        };
        let h = agent_card(&mut cv, place, n1, ctx, &view);
        assert_eq!(h, 10, "B frame {key}: the card's rows");
        assert_at(
            &format!("B n1 {key}"),
            &want,
            &cv,
            (93, 3),
            93..137,
            &(3..13).collect::<Vec<_>>(),
        );
    }
}

#[test]
fn turn_8_running_filter_keeps_waiting_and_drops_stale() {
    let (swarm, frames) = turn_8();
    for (key, panes) in frames {
        let view = states_view(
            key.parse().unwrap(),
            ("w1:p4", Some("p214"), Some("n1")),
            Filter::Running,
        );
        let orchs = orch_column(&swarm, &view, 28);
        // 6 cards: the stale ones are gone
        assert_at(
            &format!("R {key}"),
            &golden(&panes["R"]),
            &orchs.canvas,
            (2, 3),
            2..30,
            &orch_rows(33),
        );
    }
}

// -- the app's first wide screen and the deep thread ---------------------------------------

/// A screen golden's run: the screen, its swarm (a captured wire swarm), and the view with
/// the unseen marks it was drawn with.
fn screen(file: &str, snap: &str) -> (Vec<Vec<Shown>>, Swarm, View) {
    let f = fixture(&format!("screens/{file}"));
    let run = &f["runs"][0];
    let swarm = wire_swarm(run["input"].as_str().unwrap(), None);
    let view = View {
        orch: run["focus_pane"].as_str().map(Into::into),
        now: run["now"].as_f64().unwrap(),
        unseen: serde_json::from_value(f["unseen"][snap].clone()).unwrap(),
        ..View::default()
    };
    (golden(&f["screens"][snap]), swarm, view)
}

fn page_rows() -> Vec<usize> {
    (3..41).collect() // between the line above the columns and the legend
}

#[test]
fn the_demos_orchestrator_and_thread_columns() {
    let (want, swarm, mut view) = screen("demo-wide.json", "initial");
    default_children(&swarm, &mut view);
    let orchs: Column = orch_column(&swarm, &view, 28);
    assert!(orchs.height <= 38, "the column fits: not scrolled");
    assert_at(
        "demo orchestrators",
        &want,
        &orchs.canvas,
        (2, 3),
        2..30,
        &page_rows(),
    );
    let o = find_orch(&swarm, view.orch.as_deref()).unwrap();
    let threads = thread_column(o, &view, 50);
    assert_at(
        "demo threads",
        &want,
        &threads.canvas,
        (36, 3),
        36..86,
        &page_rows(),
    );
}

#[test]
fn the_deep_thread_column() {
    let (want, swarm, mut view) = screen("deep.json", "thread");
    view.thread = Some("x:deep".into());
    view.focus = 1;
    let o = find_orch(&swarm, Some("w1:p3")).unwrap();
    let threads = thread_column(o, &view, 50);
    assert_at(
        "deep thread",
        &want,
        &threads.canvas,
        (36, 3),
        36..86,
        &page_rows(),
    );
    assert_eq!(threads.anchors, [1]);
    assert_eq!((threads.sel, threads.sel_anchor), (Some((0, 10)), Some(1)));
}

#[test]
fn the_deep_threads_first_agent_folded_with_its_children_off_to_the_right() {
    // only depth 1 fits at 160 columns with the focus on the thread: l1's card says how many
    // agents it spawned, off to the right
    let (want, swarm, mut view) = screen("deep.json", "thread");
    view.thread = Some("x:deep".into());
    view.focus = 1;
    let t = find_thread(find_orch(&swarm, Some("w1:p3")), Some("x:deep")).unwrap();
    let l1 = find_agent(Some(t), Some("l1")).unwrap();
    let kids = t
        .agents
        .iter()
        .filter(|a| a.parent.as_deref() == Some("l1"))
        .count();
    let mut cv = Screen::new(44, 3);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: false,
        focus: false,
    };
    let ctx = AgentCtx {
        t,
        inbox_ok: swarm.inbox_ok,
        on_path: false,
        kids_off: kids,
    };
    assert_eq!(agent_card(&mut cv, place, l1, ctx, &view), 3);
    assert_at("deep l1", &want, &cv, (93, 3), 93..137, &[3, 4, 5]);
    assert!(cv.text().contains(&format!("› {kids}")));
}

// -- the columns' rules on a swarm built for them (tests/fixtures/graph_rules.json) ------

/// The first `golden.len()` rows of a canvas against them, cell for cell.
fn assert_rows(name: &str, want: &Value, canvas: &Screen) {
    let want = golden(want);
    let got = cells(canvas);
    assert_same(name, &want, &got[..want.len()]);
}

#[test]
fn the_columns_rules_draw_what_python_drew() {
    // captured by tests/fixtures/capture_graph_rules.py from Python's own columns
    let rules = fixture("graph_rules.json");
    let swarm: Swarm = serde_json::from_value(rules["swarm"].clone()).unwrap();
    let mut view = View {
        orch: Some("w1:p1".into()),
        thread: Some("t1".into()),
        agent: Some("n1".into()),
        focus: 2,
        now: rules["now"].as_f64().unwrap(),
        unseen: serde_json::from_value(rules["unseen"].clone()).unwrap(),
        ..View::default()
    };
    view.done_open.insert("w1:p1".into());
    // questions no agent filed (?2), a long name cut, active/live counts without the
    // session's thread and its own agent (1/11)
    let orchs = orch_column(&swarm, &view, 28);
    assert_eq!(orchs.height as usize, golden(&rules["orchs"]).len());
    assert_rows("orchestrators", &rules["orchs"], &orchs.canvas);
    // the background card without a turn end, pods past the room (+2), nested ones
    // (+1 nested), a thread never started (queued), the done threads unfolded (▾) with an
    // unseen finished agent (the folded card's quiet pulse)
    let o = find_orch(&swarm, Some("w1:p1")).unwrap();
    let threads = thread_column(o, &view, 50);
    assert_eq!(threads.height as usize, golden(&rules["threads"]).len());
    assert_rows("threads", &rules["threads"], &threads.canvas);
    // waiting on its oldest background task (30m), its phase counted from the end
    let t = find_thread(Some(o), Some("t1")).unwrap();
    let mut cv = Screen::new(44, 20);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: true,
        focus: true,
    };
    let ctx = AgentCtx {
        t,
        inbox_ok: true,
        on_path: false,
        kids_off: 0,
    };
    let h = agent_card(
        &mut cv,
        place,
        find_agent(Some(t), Some("n1")).unwrap(),
        ctx,
        &view,
    );
    assert_eq!(h as usize, golden(&rules["n1"]).len());
    assert_rows("n1", &rules["n1"], &cv);
    // a card on the selected agent's path: its title and border as selected
    let mut cv = Screen::new(44, 3);
    let place = Place {
        is_sel: false,
        focus: false,
        ..place
    };
    let ctx = AgentCtx {
        on_path: true,
        ..ctx
    };
    agent_card(
        &mut cv,
        place,
        find_agent(Some(t), Some("a0")).unwrap(),
        ctx,
        &view,
    );
    assert_rows("a0 on the path", &rules["a0"], &cv);
}
