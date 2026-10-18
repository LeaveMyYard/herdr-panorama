//! The card pieces' rules, case by case (the narrow goldens cover them as drawn).

use super::actions::actions;
use super::boxes::jn;
use super::details::agent_detail_lines;
use super::edge::sweep_pos;
use super::phases::cur_idx;
use super::pods::{Pods, pods_row};
use crate::model::{Agent, Thread};
use crate::view::palette::LINE;
use crate::view::screen::Screen;
use crate::view::state::View;
use serde_json::{Value, json};

fn thread(v: Value) -> Thread {
    let mut t = json!({"id": "t", "name": "n", "source": "turn"});
    t.as_object_mut()
        .unwrap()
        .extend(v.as_object().unwrap().clone());
    serde_json::from_value(t).unwrap()
}

fn agent(v: Value) -> Agent {
    serde_json::from_value(v).unwrap()
}

fn phases(states: &[&str]) -> Value {
    states
        .iter()
        .enumerate()
        .map(|(i, s)| json!({"name": format!("p{i}"), "state": s}))
        .collect()
}

fn text(lines: &[Vec<crate::view::seg::Seg>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.iter().map(|s| s.text.as_str()).collect())
        .collect()
}

#[test]
fn a_current_phase_with_an_end_still_counts_from_its_start() {
    // Python's phase_sub: `now` uses now - start even when an end is set
    let t =
        thread(json!({"phases": [{"name": "p", "state": "now", "start": 100.0, "end": 200.0}]}));
    assert_eq!(
        super::phases::phase_sub(
            &t,
            &View {
                now: 1000.0,
                ..View::default()
            }
        ),
        "p · 15m"
    );
}

#[test]
fn the_current_phase() {
    assert_eq!(cur_idx(&thread(json!({}))), None);
    assert_eq!(
        cur_idx(&thread(json!({"phases": phases(&["done", "now", "next"])}))),
        Some(1)
    );
    assert_eq!(
        cur_idx(&thread(
            json!({"phases": phases(&["done", "failed", "now"])})
        )),
        Some(1)
    );
    assert_eq!(
        cur_idx(&thread(
            json!({"phases": phases(&["done", "skipped", "done"])})
        )),
        Some(2)
    );
    assert_eq!(
        cur_idx(&thread(
            json!({"phases": phases(&["done", "next", "next"])})
        )),
        Some(0)
    );
}

#[test]
fn details_cap_questions_at_four_lines_and_the_last_message_at_two() {
    let long = "word ".repeat(40);
    let a = agent(json!({"id": "a", "status": "wait", "questions": [long], "activity": "asking"}));
    let lines = text(&agent_detail_lines(&a, 30, 0.0, 0.0));
    assert_eq!(lines[0], "now  asking");
    assert_eq!(lines.len(), 1 + 4);
    assert!(lines[1].starts_with("? ") && lines[2].starts_with("  "));
    let a = agent(json!({"id": "a", "status": "done", "last_message": long}));
    let lines = text(&agent_detail_lines(&a, 30, 0.0, 0.0));
    assert_eq!(lines[0], "now  done"); // no activity: the status's words
    assert_eq!(lines.len(), 1 + 2);
    assert!(lines[1].starts_with('“'));
    // a question or an error hides the last message
    let a = agent(json!({"id": "a", "status": "fail", "last_message": "m", "error": "boom"}));
    assert_eq!(
        text(&agent_detail_lines(&a, 30, 0.0, 0.0)),
        ["now  failed", "✗ boom"]
    );
    let a = agent(
        json!({"id": "a", "status": "blk", "pending": ["wants Bash(rm -rf x)", "Pick one?"]}),
    );
    assert_eq!(
        text(&agent_detail_lines(&a, 40, 0.0, 0.0))[1..],
        ["! wants Bash(rm -rf x)", "! Pick one?"]
    );
}

#[test]
fn pods_show_live_first_and_fold_the_finished_when_short_of_room() {
    let ags: Vec<Value> = (0..3)
        .map(|i| json!({"id": format!("r{i}"), "status": "run", "started": f64::from(i)}))
        .chain(
            (0..5)
                .map(|i| json!({"id": format!("d{i}"), "status": "done", "started": f64::from(i)})),
        )
        .collect();
    let t = thread(json!({"agents": ags}));
    let mut s = Screen::new(40, 1);
    pods_row(
        &mut s,
        0,
        0,
        &t,
        &View::default(),
        Pods {
            max_n: 6,
            ..Pods::default()
        },
    );
    // 3 live, then room for 3 finished but 5 of them: 2 shown, "+3 done"
    assert_eq!(s.text().trim_end(), " ⠋   ⠋   ⠋   ✓   ✓  +3 done");
    let mut s = Screen::new(40, 1);
    pods_row(
        &mut s,
        0,
        0,
        &t,
        &View::default(),
        Pods {
            max_n: 2,
            ..Pods::default()
        },
    );
    assert_eq!(s.text().trim_end(), " ⠋   ⠋  +6");
}

#[test]
fn actions_close_their_gaps_then_shorten_in_a_narrow_card() {
    let a = agent(json!({"id": "a", "status": "blk"}));
    let line = |room: Option<i32>| {
        let mut s = Screen::new(60, 1);
        actions(&mut s, 0, 0, &a, true, None, room);
        s.text().trim_end().to_owned()
    };
    assert_eq!(line(None), "g open pane   t transcript   a answer in pane");
    assert_eq!(
        line(Some(44)),
        "g open pane  t transcript  a answer in pane"
    );
    assert_eq!(line(Some(42)), "g open pane  t transcript  a answer");
    assert_eq!(line(Some(30)), "g open pane  t transcript  a answer");
}

#[test]
fn lines_merge_into_tees_and_join_an_animated_edge() {
    let mut s = Screen::new(3, 1);
    s.put(1, 0, "│", LINE, None, false);
    jn(&mut s, 1, 0, 8, LINE);
    assert_eq!(s.cell(1, 0).ch, '├');
    s.put(2, 0, "┃", LINE, None, false);
    jn(&mut s, 2, 0, 4, LINE);
    assert_eq!(s.cell(2, 0).ch, '┨');
    s.put(2, 0, "┃", LINE, None, false);
    jn(&mut s, 2, 0, 8, LINE);
    assert_eq!(s.cell(2, 0).ch, '┠');
    jn(&mut s, 0, 0, 12, LINE);
    assert_eq!(s.cell(0, 0).ch, '─');
}

#[test]
fn the_sweep_laps_in_at_most_48_frames() {
    // a 44-column card: one cell a frame; a wider one moves further each frame
    assert_eq!(sweep_pos(0.0, 0, 42), -3);
    assert_eq!(sweep_pos(1.0, 0, 42), -2);
    assert_eq!(sweep_pos(48.0, 0, 42), -3);
    assert_eq!(sweep_pos(1.0, 0, 90), -1); // 96 cells over 48 frames: 2 a frame
    assert_eq!(sweep_pos(0.0, 1, 42), 4); // seed 1: 7 frames on
}

#[test]
fn tags_sit_right_aligned_on_the_border_and_skip_empty_ones() {
    use super::tags::tags;
    let mut s = Screen::new(30, 3);
    let items = [
        Some("42m".into()),
        None,
        Some(String::new()),
        Some("wt/x".into()),
    ];
    tags(&mut s, 0, 2, 30, &items);
    assert_eq!(s.text(), "\n\n                 42m   wt/x");
}

#[test]
fn fit_segs_cut_a_title_across_its_segments() {
    use crate::view::palette::{DIM, FAINT, FG};
    use crate::view::seg::{Seg, fit_segs};
    let title = vec![
        Seg::new("general", FG).bold(),
        Seg::new(" 1234567", FAINT),
        Seg::new(" · opus", DIM),
    ];
    let cut = fit_segs(title.clone(), 12);
    assert_eq!(
        cut,
        vec![Seg::new("general", FG).bold(), Seg::new(" 123…", FAINT)]
    );
    assert_eq!(fit_segs(title.clone(), 99), title);
    assert_eq!(
        fit_segs(vec![Seg::new("abc", FG)], 0),
        vec![Seg::new("…", FG)]
    );
}

#[test]
fn marks_count_the_questions_no_agent_filed() {
    use super::marks::marks_of;
    let a = agent(json!({"id": "a", "status": "wait", "question_ids": [1, 2]}));
    let words = |extra| -> Vec<String> {
        marks_of(&[&a], false, extra)
            .into_iter()
            .map(|m| m.text)
            .collect()
    };
    assert_eq!(words(0), ["?2"]);
    assert_eq!(words(3), ["?5"]);
    let idle = agent(json!({"id": "b", "status": "done"}));
    let loose: Vec<String> = marks_of(&[&idle], false, 1)
        .into_iter()
        .map(|m| m.text)
        .collect();
    assert_eq!(loose, ["?1"]);
}

#[test]
fn top_only_pods_are_the_threads_own_then_nested() {
    // Python's pods_row(top_only=True) on the same thread (captured): the pod on the
    // selected agent's path is bracketed
    use crate::view::target::Target;
    let t = thread(json!({"agents": [
        {"id": "a", "status": "run"}, {"id": "b", "status": "done"},
        {"id": "c", "parent": "a", "status": "run"}, {"id": "d", "parent": "c", "status": "run"},
    ]}));
    let mut s = Screen::new(40, 1);
    let p = Pods {
        max_n: 6,
        sel_agent: Some("d"),
        orch: Some("p"),
        top_only: true,
    };
    pods_row(&mut s, 1, 0, &t, &View::default(), p);
    assert_eq!(s.text(), "[ ⠋ ] ✓  +2 nested");
    let agents: Vec<(i32, i32, String)> = s
        .targets
        .iter()
        .map(|h| match &h.target {
            Target::Agent(r) => (h.x0, h.x1, r.agent.clone()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(agents, [(1, 4, "a".into()), (5, 8, "b".into())]);
    let mut s = Screen::new(40, 1);
    let p = Pods {
        max_n: 1,
        top_only: true,
        ..Pods::default()
    };
    pods_row(&mut s, 1, 0, &t, &View::default(), p);
    assert_eq!(s.text(), "  ⠋  +1 done +2 nested");
}

#[test]
fn a_sweep_lap_is_at_most_12_seconds_and_narrow_cards_are_unchanged() {
    // test_wide.py::test_a_lap_is_at_most_12_seconds_and_narrow_cards_are_unchanged
    use super::Area;
    use super::edge::{SWEEP_LAP, edge};
    use crate::model::Status;
    use crate::view::orch_state::OrchState;
    assert_eq!(SWEEP_LAP, 48); // 250 ms frames
    let working = OrchState {
        st: Status::Running,
        glyph: "⠋".into(),
        label: "working".into(),
        working: true,
        bg: 0,
    };
    let bar_start = |w: i32, frame: i64| {
        let mut s = Screen::new(w, 3);
        edge(
            &mut s,
            Area {
                x: 0,
                y: 0,
                w,
                h: 3,
            },
            &working,
            frame as f64,
            0,
        );
        (0..w).find(|x| s.cell(*x, 2).ch == '━')
    };
    for w in [44, 56, 60, 64] {
        for f in 0..48 {
            assert_eq!(bar_start(w, f), bar_start(w, f + 48), "{w} {f}"); // one lap: 48
        }
    }
    for f in 0..32 {
        assert_eq!(bar_start(28, f), bar_start(28, f + 32)); // as before
    }
}

#[test]
fn the_pulse_runs_over_the_whole_border_capped_on_a_selected_card() {
    use super::Area;
    use super::pulse::{SELECTED_PULSE, pulse_box};
    use crate::view::anim::pulse;
    use crate::view::palette::{DIM, HI};
    use crate::view::unseen::{Mark, Unseen};
    let m = Mark {
        event: Unseen::Fail,
        quiet: false,
    };
    let card = Area {
        x: 1,
        y: 0,
        w: 5,
        h: 3,
    };
    for (base, cap) in [(LINE, 1.0), (DIM, SELECTED_PULSE), (HI, SELECTED_PULSE)] {
        let mut s = Screen::new(8, 4);
        super::boxes::rbox(&mut s, 1, 0, 5, 3, base);
        pulse_box(&mut s, card, Some(m), 6.0, base);
        let want = pulse(6.0, m, base, cap);
        let border = [
            (1, 0),
            (5, 0),
            (3, 0),
            (1, 1),
            (5, 1),
            (1, 2),
            (5, 2),
            (3, 2),
        ];
        for (x, y) in border {
            assert_eq!(
                (s.cell(x, y).fg, s.cell(x, y).fg_moves),
                (want, true),
                "{x},{y}"
            );
        }
        assert!(!s.drawn(3) && s.cell(3, 1).ch == ' ');
    }
    let mut s = Screen::new(8, 4);
    pulse_box(&mut s, card, None, 6.0, LINE);
    assert!(!s.drawn(0));
}
