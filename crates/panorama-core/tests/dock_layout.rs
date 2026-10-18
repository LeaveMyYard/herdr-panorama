//! The docked panel's click targets and layout rules: Python's tests/test_dock_panel.py (its
//! render-level cases) and tests/test_glyph_spacing.py's docked rows, ported.

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::{NEW_ROW, Screen, Target, View, render_dock};
use serde_json::json;
use support::design::design_swarm;
use support::{NO_SESSIONS, fixture};

/// design_dock.design_swarm(): the design's dataset (design_data.json), docked next to tab 3.
fn swarm() -> Swarm {
    let mut sw = design_swarm(&fixture("design_data.json"));
    sw.docked = Some("w1:p3".into());
    sw
}

/// test_dock_panel.py `design_view`: tab 3 selected, the mouse over Billing Migrations.
fn design_view(frame: f64) -> View {
    View {
        orch: Some("w1:p3".into()),
        hover: Some(Target::Orch("w1:p4".into())),
        frame,
        ..View::default()
    }
}

fn lines(s: &Screen) -> Vec<String> {
    s.text().lines().map(str::to_owned).collect()
}

fn orch(p: &str) -> Target {
    Target::Orch(p.into())
}

#[test]
fn every_card_is_a_click_target_and_question_marks_answer() {
    // test_every_card_is_a_click_target_and_question_marks_answer
    let s = render_dock(&swarm(), &design_view(4.0), 46, 45, None, NO_SESSIONS);
    let kinds: Vec<&Target> = s.targets.iter().map(|h| &h.target).collect();
    for n in 1..=4 {
        assert!(kinds.contains(&&orch(&format!("w1:p{n}"))), "w1:p{n}");
    }
    assert!(kinds.contains(&&Target::NewAgent));
    assert!(kinds.contains(&&Target::AnswerOrch("w1:p1".into())));
    assert!(kinds.contains(&&Target::AnswerOrch("w1:p3".into())));
    let xq = lines(&s)[2].chars().collect::<Vec<_>>();
    let xq = (0..xq.len())
        .find(|&x| xq[x] == '?' && xq.get(x + 1) == Some(&'1'))
        .unwrap() as i32;
    let answer = Some(Target::AnswerOrch("w1:p1".into()));
    assert_eq!(s.target_at(xq, 2).cloned(), answer); // "?1" on Payments UI
    assert_eq!(s.target_at(xq + 1, 2).cloned(), answer);
    assert_eq!(s.target_at(xq + 3, 2), Some(&orch("w1:p1")));
    assert_eq!(s.target_at(20, 3), Some(&orch("w1:p1"))); // the second line too
    assert_eq!(s.target_at(20, 12), Some(&orch("w1:p3"))); // the docked one's thread list
    assert_eq!(s.target_at(20, 4), None); // the gap between entries
}

#[test]
fn the_docked_panel_scrolls_to_the_cursor() {
    // test_docked_panel_scrolls_to_the_cursor
    let v = View {
        orch: Some(NEW_ROW.into()),
        hover: None,
        ..design_view(0.0)
    };
    let s = render_dock(&swarm(), &v, 46, 16, None, NO_SESSIONS);
    let text = s.text();
    assert!(text.contains("New agent"));
    let rows = lines(&s);
    assert!(rows[0].contains("SWARM") && rows.last().unwrap().contains("move"));
}

/// The state labels the docked rows end with (test_dock_panel.py `STATES`).
const STATES: [&str; 8] = [
    "failed",
    "blocked",
    "working",
    "needs you",
    "waiting",
    "stale",
    "idle",
    "done",
];

#[test]
fn the_panel_lays_out_to_the_width_it_gets() {
    // test_3a_lays_out_to_the_width_it_gets[36, 40, 44, 46, 50, 60]
    let sw = swarm();
    for w in [36, 40, 44, 46, 50, 60] {
        let v = View {
            zoom_hint: true,
            ..design_view(4.0)
        };
        let rows = lines(&render_dock(&sw, &v, w, 45, None, NO_SESSIONS));
        for o in &sw.orchestrators {
            let tag = format!(" {} ", o.tab_no.unwrap());
            let row = rows
                .iter()
                .find(|r| r.chars().take(5).collect::<String>().contains(&tag))
                .unwrap_or_else(|| panic!("{w}: no row for tab {tag}"));
            let row = row.trim_end();
            assert!(STATES.iter().any(|st| row.ends_with(st)), "{w}: {row}");
            assert_eq!(row.chars().count() as i32, w - 1, "{w}: {row}");
        }
        assert!(rows[0].trim_end().ends_with("?3 !1 ✗1"), "{w}");
        let foot = rows.last().unwrap().trim_end();
        assert!(
            foot.ends_with("back") && (foot.chars().count() as i32) < w,
            "{w}: {foot}"
        );
        assert!(rows[rows.len() - 2].contains("z graph"), "{w}");
        assert!(
            rows.iter()
                .any(|r| r.contains("New agent") && r.trim_end().ends_with('n'))
        );
    }
}

/// test_glyph_spacing.py `many_threads`: one orchestrator with `n` running threads.
fn many_threads(n: usize) -> Swarm {
    let threads: Vec<_> = (0..n)
        .map(|i| {
            json!({"id": format!("t{i}"), "name": format!("thread {i}"), "source": "turn",
            "agents": [{"id": format!("a{i}"), "status": "run", "seq": i}]})
        })
        .collect();
    serde_json::from_value(json!({"orchestrators": [{"pane_id": "w1:p1", "tab_id": "t",
        "tab_label": "busy", "session_id": "s", "tab_no": 1, "threads": threads}]}))
    .unwrap()
}

/// An entry's second line: its thread tiles and a "+n", before the counts.
fn line_two(row: &str) -> Vec<String> {
    let head = row.split(" active · ").next().unwrap();
    let head = head.rsplit_once("  ").map_or(head, |(a, _)| a);
    head.split_whitespace().map(str::to_owned).collect()
}

fn counts_row(s: &Screen) -> String {
    lines(s)
        .into_iter()
        .find(|r| r.contains("active · "))
        .unwrap()
}

#[test]
fn many_threads_keep_the_counts_whole() {
    // test_review_m_many_threads_keep_the_counts_whole[15/30 × 36/46]
    for n in [15, 30] {
        for w in [36, 46] {
            let v = View {
                orch: Some("w1:p1".into()),
                ..View::default()
            };
            let s = render_dock(&many_threads(n), &v, w, 45, None, NO_SESSIONS);
            let row = counts_row(&s);
            let row = row.trim_end();
            assert!(
                row.ends_with(&format!("{n} active · {n} agents")),
                "{n} {w}: {row}"
            );
            assert!(row.chars().count() as i32 <= w);
            let shown = line_two(row);
            let last = shown.last().unwrap();
            assert!(last.starts_with('+'), "{n} {w}: {row}");
            assert_eq!(shown.len() - 1 + last[1..].parse::<usize>().unwrap(), n);
            assert_eq!(support::glyphs::adjacent(&s), Vec::<(i32, i32)>::new());
        }
    }
}

#[test]
fn a_few_threads_show_every_glyph() {
    // test_a_few_threads_show_every_glyph
    let v = View {
        orch: Some("w1:p1".into()),
        ..View::default()
    };
    let row = counts_row(&render_dock(
        &many_threads(3),
        &v,
        46,
        45,
        None,
        NO_SESSIONS,
    ));
    assert!(!row.contains('+') && line_two(&row).len() == 3, "{row}");
}

#[test]
fn the_cursor_has_no_highlight_only_the_mouse_has() {
    // test_panel_sync.py::test_the_cursor_has_no_highlight_only_the_mouse_has: with the
    // cursor look off (the per-tab panels), a selected row that is not the docked one keeps
    // the plain background until the mouse is over it
    use panorama_core::view::palette::DOCK_HOV;
    let sw = swarm();
    let other = sw
        .orchestrators
        .iter()
        .find(|o| sw.docked.as_deref() != Some(o.pane_id.as_str()))
        .unwrap();
    let mut v = View {
        cursor_look: false,
        orch: Some(other.pane_id.clone()),
        hover: None,
        ..design_view(4.0)
    };
    let s = render_dock(&sw, &v, 46, 45, None, NO_SESSIONS);
    let tag = format!(" {} ", other.tab_no.unwrap());
    let y = lines(&s)
        .iter()
        .position(|r| r.chars().take(5).collect::<String>().contains(&tag))
        .unwrap() as i32;
    assert_ne!(s.cell(30, y).bg, Some(DOCK_HOV));
    v.hover = Some(orch(&other.pane_id));
    let s = render_dock(&sw, &v, 46, 45, None, NO_SESSIONS);
    assert_eq!(s.cell(30, y).bg, Some(DOCK_HOV));
}
