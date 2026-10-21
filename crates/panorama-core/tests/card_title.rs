//! An agent card's title (role, id, model) never runs into its right-aligned state label, at
//! any card width: it is cut with "…" (Python tests/test_card_title.py; the operator saw
//! `· opurunning`).

mod support;

use panorama_core::model::{Status, Swarm};
use panorama_core::view::palette::{DIM, FAINT, FG};
use panorama_core::view::seg::{Seg, fit_segs};
use panorama_core::view::{View, render_graph};
use serde_json::{Value, json};
use support::NO_SESSIONS;

/// The agent and a second one in thread `t` of orchestrator w1:p1 (`swarm_with`).
fn swarm_with(agent: Value) -> Swarm {
    let other = json!({"id": "b2c3d4e", "role": "other", "status": "run"});
    serde_json::from_value(json!({"orchestrators": [{
        "pane_id": "w1:p1", "tab_id": "t1", "tab_label": "orch", "session_id": "s",
        "threads": [{"id": "t", "name": "work", "source": "turn", "agents": [agent, other]}],
    }]}))
    .unwrap()
}

fn rows(sw: &Swarm, v: &View, w: i32) -> Vec<String> {
    render_graph(sw, v, w, 45, None, NO_SESSIONS)
        .text()
        .split('\n')
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_selected_title_stops_before_the_state() {
    // test_the_selected_title_stops_before_the_state[run|done-120|140|160|200|320]
    for (code, status) in [("run", Status::Running), ("done", Status::Done)] {
        for w in [120, 140, 160, 200, 320] {
            let ended = (code == "done").then_some(60.0);
            let a = json!({"id": "a6c8a7b1", "role": "general-purpose", "model": "opus 4.5", "status": code,
                           "started": 0.0, "ended": ended});
            let v = View {
                orch: Some("w1:p1".into()),
                thread: Some("t".into()),
                agent: Some("a6c8a7b1".into()),
                focus: 2,
                now: 3600.0,
                fin_open: ["t".to_owned()].into(),
                ..View::default()
            };
            let rows = rows(&swarm_with(a), &v, w);
            let row = rows
                .iter()
                .find(|r| r.contains("general-pur") && !r.contains("finished"))
                .expect("the card's title row");
            let label = status.label();
            let end = row
                .find(label)
                .unwrap_or_else(|| panic!("{code} {w}: {label} in {row:?}"));
            assert_eq!(row[..end].chars().last(), Some(' '), "{code} {w}: {row:?}");
            assert!(
                !row.contains("opurunning") && !row.contains("opudone"),
                "{row:?}"
            );
            let head = row[..end].trim_end();
            assert!(
                head.ends_with("opus 4.5") || head.ends_with('…'),
                "{code} {w}: {row:?}"
            );
        }
    }
}

#[test]
fn an_unselected_title_is_cut_too() {
    // test_an_unselected_title_is_cut_too[120|160]
    for w in [120, 160] {
        let a = json!({"id": "a6c8a7b1", "role": "general-purpose-very-long-agent-type", "model": "sonnet 4.5", "status": "run"});
        let v = View {
            orch: Some("w1:p1".into()),
            thread: Some("t".into()),
            agent: Some("b2c3d4e".into()),
            focus: 2,
            now: 3600.0,
            ..View::default()
        };
        let rows = rows(&swarm_with(a), &v, w);
        let row = rows
            .iter()
            .find(|r| r.contains("general-purpose"))
            .expect("the card's title row");
        let chars: Vec<char> = row.chars().collect();
        let at = chars
            .iter()
            .collect::<String>()
            .find("general-purpose")
            .unwrap();
        let at = row[..at].chars().count();
        let card: Vec<char> = chars[at.saturating_sub(6)..].to_vec();
        let border = card
            .iter()
            .skip(1)
            .position(|c| *c == '│')
            .map_or(card.len(), |p| p + 1);
        assert!(card[..border].contains(&'…'), "{w}: {row:?}");
    }
}

#[test]
fn fit_segs_keeps_short_titles_and_cuts_long_ones() {
    // test_fit_segs_keeps_short_titles_and_cuts_long_ones
    let segs = vec![
        Seg::new("general-purpose", FG),
        Seg::new(" a6c8a7b", FAINT),
        Seg::new(" · opus", DIM),
    ];
    assert_eq!(fit_segs(segs.clone(), 40), segs);
    let cut = fit_segs(segs, 20);
    assert_eq!(
        cut.iter().map(|s| s.text.as_str()).collect::<String>(),
        "general-purpose a6c…"
    );
    assert_eq!(cut.iter().map(|s| s.fg).collect::<Vec<_>>(), [FG, FAINT]);
}

#[test]
fn a_title_that_fits_exactly_is_kept_whole() {
    // the edge of fit_segs: a title of exactly `room` cells stays, one cell less cuts it
    let segs = vec![
        Seg::new("general-purpose", FG),
        Seg::new(" a6c8a7b", FAINT),
        Seg::new(" · opus", DIM),
    ];
    let full: String = segs.iter().map(|s| s.text.as_str()).collect();
    let n = full.chars().count() as i32;
    assert_eq!(fit_segs(segs.clone(), n), segs);
    let cut: String = fit_segs(segs, n - 1)
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(cut.chars().count() as i32, n - 1);
    assert!(
        cut.ends_with('…') && full.starts_with(cut.trim_end_matches('…')),
        "{cut:?}"
    );
}
