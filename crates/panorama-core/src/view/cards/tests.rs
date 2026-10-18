//! The card pieces' rules, case by case (the narrow goldens cover them as drawn).

use super::actions::actions;
use super::boxes::{LINE_COL, jn};
use super::details::agent_detail_lines;
use super::edge::sweep_pos;
use super::phases::cur_idx;
use super::pods::{Pods, pods_row};
use crate::model::{Agent, Thread};
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
    s.put(1, 0, "│", LINE_COL, None, false);
    jn(&mut s, 1, 0, 8, LINE_COL);
    assert_eq!(s.cell(1, 0).ch, '├');
    s.put(2, 0, "┃", LINE_COL, None, false);
    jn(&mut s, 2, 0, 4, LINE_COL);
    assert_eq!(s.cell(2, 0).ch, '┨');
    jn(&mut s, 0, 0, 12, LINE_COL);
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
