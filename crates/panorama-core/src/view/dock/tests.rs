use super::*;
use crate::model::Status;
use crate::view::palette::{FAINT, FG};
use crate::view::theme::PanelLook;

#[test]
fn the_theme_leaves_animated_colours_as_drawn() {
    // a needs-you glyph in its off phase is FAINT, as a plain faint text is: only the plain
    // one is the theme's (Python skips Anim colours)
    let mut s = Screen::new(3, 1);
    s.fill_bg(0, 0, 3, 1, BG);
    s.put(0, 0, "a", FAINT, None, false);
    s.put_seg(1, 0, &Seg::new("!", FAINT).fg_moving(true));
    s.fill_bg_moving(2, 0, 1, 1, BG, true); // a tint that happens to land on the background
    let to = crate::view::Rgb::hex(0x565f89);
    let look = PanelLook {
        bg: crate::view::Rgb::hex(0x1a1b26),
        fgs: vec![(FAINT, to), (FG, to)],
        hover: BG,
    };
    look::apply(&mut s, &look);
    assert_eq!((s.cell(0, 0).fg, s.cell(0, 0).bg), (to, Some(look.bg)));
    assert_eq!((s.cell(1, 0).fg, s.cell(1, 0).bg), (FAINT, Some(look.bg)));
    assert_eq!(s.cell(2, 0).bg, Some(BG)); // a moving background: untouched
}

#[test]
fn the_footer_fills_in_the_open_key_and_drops_whole_hints_when_narrow() {
    let line = |w: i32, key: (&str, &str)| {
        let v = View {
            open_key: (key.0.into(), key.1.into()),
            ..View::default()
        };
        let mut s = Screen::new(w, 3);
        chrome::footer(&mut s, &v, true);
        s.text().lines().last().unwrap().to_owned()
    };
    assert_eq!(
        line(46, ("pfx+spc", "pfx+spc")),
        " ↑↓ move ret open a answer n new pfx+spc back"
    );
    // a long key: whole without `n new`, then its short form
    assert_eq!(
        line(46, ("ctrl+b space", "^b spc")),
        " ↑↓ move ret open a answer ctrl+b space back"
    );
    assert_eq!(
        line(40, ("ctrl+b space", "^b spc")),
        " ↑↓ move ret open a answer ^b spc back"
    );
    assert_eq!(
        line(30, ("pfx+spc", "pfx+spc")),
        " ↑↓ move ret open pfx+spc back"
    );
}

#[test]
fn many_thread_tiles_end_in_plus_n_and_the_counts_stay_whole() {
    let t = |g: &str| tiles::tile(&Status::Running, g, 0.0, None);
    let line = tiles::fit_tiles(vec![t("a"), t("b"), t("c"), t("d")], "  4 active", 20);
    let text: String = line.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, " a   b  +2  4 active");
    let line = tiles::fit_tiles(vec![t("a")], "  1 active · 9 agents", 8);
    assert_eq!(
        line.iter().map(|s| s.text.as_str()).collect::<String>(),
        "1 activ…"
    );
}

fn orch(extra: serde_json::Value) -> Orchestrator {
    let mut v =
        serde_json::json!({"pane_id": "p", "tab_id": null, "tab_label": "x", "session_id": null});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn thread(status: &str, agents: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"id": "t", "name": "n", "source": "turn", "status": status, "agents": agents})
}

const NOW: f64 = 100_000.0;
const AGES: Ages = Ages {
    now: NOW,
    stale_after: 3600.0,
};

#[test]
fn the_entry_states_ladder_in_pythons_order() {
    let done = || thread("done", serde_json::json!([{"id": "a", "status": "done"}]));
    let bg_at = |t: f64| serde_json::json!([{"id": "b", "kind": "k", "started": t}]);
    // (case, orchestrator, status, label)
    let cases = [
        (
            "failed wins over blocked",
            orch(
                serde_json::json!({"threads": [thread("active", serde_json::json!(
            [{"id": "a", "status": "blk"}, {"id": "b", "status": "fail"}]))]}),
            ),
            Status::Failed,
            "failed",
        ),
        (
            "herdr blocked",
            orch(serde_json::json!({"herdr_status": "blocked"})),
            Status::Blocked,
            "blocked",
        ),
        (
            "all done but a question",
            orch(serde_json::json!({"threads": [done()], "inbox": [1]})),
            Status::Waiting,
            "needs you",
        ),
        (
            "questions of the agents",
            orch(
                serde_json::json!({"threads": [thread("active", serde_json::json!(
            [{"id": "a", "status": "q", "question_ids": [4, 5]}]))]}),
            ),
            Status::Waiting,
            "needs you",
        ),
        (
            "all done with background work",
            orch(serde_json::json!({"threads": [done()], "bg": bg_at(NOW - 60.0)})),
            Status::Background,
            "waiting",
        ),
        (
            "waiting past stale_after says how long",
            orch(serde_json::json!({"bg": bg_at(NOW - 7200.0)})),
            Status::Background,
            "waiting 2h",
        ),
        (
            "all done and stale",
            orch(serde_json::json!({"threads": [done()], "last_activity": NOW - 7200.0})),
            Status::Stale,
            "stale 2h",
        ),
        (
            "all done",
            orch(serde_json::json!({"threads": [done()], "last_activity": NOW - 60.0})),
            Status::Done,
            "done",
        ),
        (
            "herdr working, no runner",
            orch(serde_json::json!({"herdr_status": "working"})),
            Status::Running,
            "working",
        ),
        (
            "reviewing only",
            orch(
                serde_json::json!({"threads": [thread("active", serde_json::json!(
            [{"id": "a", "status": "rev"}]))]}),
            ),
            Status::Reviewing,
            "working",
        ),
        (
            "nothing",
            orch(serde_json::json!({})),
            Status::Queued,
            "idle",
        ),
    ];
    for (case, o, st, label) in cases {
        let d = state::dock_state(&o, 3.0, AGES);
        assert_eq!((d.st, d.label.as_str()), (st, label), "{case}");
    }
    let working = orch(serde_json::json!({"herdr_status": "working"}));
    let d = state::dock_state(&working, 3.0, AGES);
    assert_eq!(d.glyph, crate::view::anim::spin(3.0, 0).to_string()); // working without a runner: the spinner
}

#[test]
fn tiles_that_fit_exactly_keep_every_tile() {
    let t = |g: &str| tiles::tile(&Status::Running, g, 0.0, None);
    // two tiles (3 cells each, one space between) and the counts: 7 + 10 = 17 cells
    let line = tiles::fit_tiles(vec![t("a"), t("b")], "  2 active", 17);
    assert_eq!(
        line.iter().map(|s| s.text.as_str()).collect::<String>(),
        " a   b   2 active"
    );
}

#[test]
fn a_name_never_takes_more_than_22_cells() {
    let o =
        orch(serde_json::json!({"tab_no": 1, "tab_label": "a-very-long-orchestrator-name-indeed"}));
    let mut cv = Screen::new(80, 2);
    let look = row::RowLook {
        docked: false,
        cursor: false,
    };
    row::dock_row(&mut cv, 0, &o, &View::default(), AGES, look, 80);
    let line = cv.text().lines().next().unwrap().to_owned();
    assert!(line.contains(" a-very-long-orchestra… "), "{line:?}");
}

#[test]
fn a_tab_number_of_zero_reads_as_unknown() {
    let o = orch(serde_json::json!({"tab_no": 0}));
    let mut cv = Screen::new(46, 2);
    row::dock_row(
        &mut cv,
        0,
        &o,
        &View::default(),
        AGES,
        row::RowLook {
            docked: false,
            cursor: false,
        },
        46,
    );
    assert!(cv.text().starts_with("  ? "), "{:?}", cv.text());
}
