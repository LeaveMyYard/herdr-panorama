//! An agent card's title (role, id, model) never runs into its right-aligned state label, at
//! any card width: it is cut with "…" (Python tests/test_card_title.py; the operator saw
//! `· opurunning`). A titled card (U1a) leads with its title; role, id and model go below.

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

/// Two titled agents (the second spawned by the first) under a titled orchestrator.
fn titled() -> Swarm {
    serde_json::from_value(json!({"orchestrators": [{
        "pane_id": "w1:p1", "tab_id": "t1", "tab_label": "orch", "session_id": "s",
        "title": "Billing API", "title_source": "tab", "tab_no": 1,
        "threads": [{"id": "t", "name": "work", "source": "turn", "agents": [
            {"id": "a6c8a7b1", "title": "Author the invoice export", "title_source": "spawner",
             "role": "general-purpose", "model": "opus 4.5", "status": "run", "started": 0.0},
            {"id": "b2c3d4e5", "title": "Review the diff", "title_source": "spawner",
             "role": "reviewer", "model": "sonnet 4.5", "status": "run", "parent": "a6c8a7b1",
             "depth": 2, "started": 0.0},
        ]}],
    }]}))
    .unwrap()
}

fn titled_view(agent: &str) -> View {
    View {
        orch: Some("w1:p1".into()),
        thread: Some("t".into()),
        agent: Some(agent.into()),
        focus: 2,
        now: 3600.0,
        ..View::default()
    }
}

#[test]
fn a_titled_card_leads_with_its_title_and_keeps_role_id_and_model_secondary() {
    let rows = rows(&titled(), &titled_view("a6c8a7b1"), 200);
    let text = rows.join("\n");
    assert!(text.contains("Claude Code · Bill…"), "{text}"); // the title, not `orch`
    let at = rows
        .iter()
        .position(|r| r.contains("Author the invoice export"))
        .expect("the selected card's title row");
    assert!(!rows[at].contains("general-purpose"), "{}", rows[at]);
    assert!(
        rows[at + 1..at + 4]
            .iter()
            .any(|r| r.contains("general-purpose a6c8a7b · opus 4.5")),
        "{text}"
    );
    // unselected: its title on its row, its model among the tags on the border below
    let kid = rows
        .iter()
        .position(|r| r.contains("Review the diff"))
        .expect("the folded card's title row");
    assert!(!rows[kid].contains("reviewer b2c3d4e"), "{}", rows[kid]);
    assert!(rows[kid + 1].contains(" sonnet 4.5 "), "{}", rows[kid + 1]);
}

#[test]
fn an_untitled_card_still_reads_role_and_id() {
    // a 1.3 core or the Python core sends no titles: the cards read as before
    let mut sw = titled();
    for a in &mut sw.orchestrators[0].threads[0].agents {
        a.title.clear();
    }
    sw.orchestrators[0].title.clear();
    let text = rows(&sw, &titled_view("b2c3d4e5"), 200).join("\n");
    assert!(text.contains("general-purpose a6c8a7b"), "{text}");
    assert!(text.contains("Claude Code · orch "), "{text}");
}

#[test]
fn narrow_and_dock_rows_show_titles() {
    let mut sw = titled();
    sw.docked = Some("w1:p1".into());
    let v = titled_view("a6c8a7b1");
    let narrow = panorama_core::view::render_narrow(&sw, &v, 70, 45, None, NO_SESSIONS).text();
    let row = narrow
        .lines()
        .find(|r| r.contains("Author the invoice export"))
        .unwrap_or_else(|| panic!("{narrow}"));
    assert!(
        row.contains("Author the invoice export general-purpose a6c8a7b"),
        "{row}"
    );
    let dock = panorama_core::view::render_dock(&sw, &v, 46, 45, None, NO_SESSIONS).text();
    assert!(dock.contains("Author the invoice"), "{dock}");
    assert!(!dock.contains("general-purpose a6c"), "{dock}");
}

#[test]
fn a_selected_titled_card_keeps_its_gap_to_the_card_below() {
    // its `role id · model` line is counted in the layout: the next card moves down by one row
    let sibling = |sw: &mut Swarm| {
        let mut b = sw.orchestrators[0].threads[0].agents[1].clone();
        b.parent = None;
        b.depth = 1;
        sw.orchestrators[0].threads[0].agents[1] = b;
    };
    let mut titled_sw = titled();
    sibling(&mut titled_sw);
    let mut plain = titled_sw.clone();
    for a in &mut plain.orchestrators[0].threads[0].agents {
        a.title.clear();
    }
    let v = titled_view("a6c8a7b1");
    let at = |sw: &Swarm, text: &str| {
        let rows = rows(sw, &v, 200);
        let at = rows.iter().position(|r| r.contains(text)).expect(text);
        (at, rows[at - 1].clone())
    };
    let (t_at, t_top) = at(&titled_sw, "Review the diff");
    let (p_at, _) = at(&plain, "reviewer b2c3d4e");
    assert_eq!(t_at, p_at + 1, "the gap row was eaten");
    assert!(t_top.contains('╭'), "{t_top}");
}
