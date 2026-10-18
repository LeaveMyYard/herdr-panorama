//! The panel's keys against the narrow app-screen golden (`screens/demo-narrow.json`,
//! captured from Python's `SwarmApp` at 70 columns): a panel whose tab has no orchestrator
//! shows the narrow layout, and its keys walk it as Python's do. Each snap after a key
//! press is drawn from the panel's own state, cell for cell.

use panorama::panel::input::Key;
use panorama::panel::model::Panel;
use panorama::panel::paint::Frame;
use panorama::proto::state::State;
use panorama_core::view::{PanelSettings, Screen};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cell(ch: char, fg: &str, bg: &str, bold: bool) -> String {
    if ch == ' ' {
        format!("[ {}]", bg.to_lowercase())
    } else {
        format!(
            "[{ch}{}{}{}]",
            fg.to_lowercase(),
            bg.to_lowercase(),
            u8::from(bold)
        )
    }
}

fn golden(runs: &Value) -> Vec<String> {
    runs.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .flat_map(|run| {
                    let (fg, bg) = (
                        run[1].as_str().unwrap_or("#c3c7cd"),
                        run[2].as_str().unwrap_or("#101215"),
                    );
                    let bold = run[3].as_bool().unwrap_or(false);
                    run[0]
                        .as_str()
                        .unwrap()
                        .chars()
                        .map(move |ch| cell(ch, fg, bg, bold))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}

fn cells(s: &Screen) -> Vec<String> {
    (0..s.h())
        .map(|y| {
            (0..s.w())
                .map(|x| {
                    let c = s.cell(x, y);
                    cell(
                        c.ch,
                        &c.fg.to_string(),
                        &c.bg.map_or("#101215".to_owned(), |b| b.to_string()),
                        c.bold,
                    )
                })
                .collect()
        })
        .collect()
}

#[test]
fn the_demos_keys_walk_the_narrow_layout_as_pythons() {
    let file = fixture("screens/demo-narrow.json");
    let run = &file["runs"][0];
    let swarm = fixture(run["input"].as_str().unwrap())["swarm"].clone();
    let t0 = Instant::now();
    let mut panel = Panel::new(None, None);
    panel.on_connected(true);
    panel.on_state(
        State::read(json!({ "swarm": swarm }).as_object().unwrap()).unwrap(),
        t0,
    );
    let f = Frame {
        now: run["now"].as_f64().unwrap(),
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: PanelSettings::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let t = t0 + Duration::from_secs(1);
    let mut snaps = 0;
    for step in run["steps"].as_array().unwrap() {
        match step["do"].as_str().unwrap() {
            "press" => {
                for k in step["keys"].as_array().unwrap() {
                    let key = match k.as_str().unwrap() {
                        "down" => Key::Move(1),
                        "up" => Key::Move(-1),
                        "right" => Key::Right,
                        "left" => Key::Left,
                        other => panic!("key {other}"),
                    };
                    assert!(panel.on_key(key, t).is_empty());
                }
            }
            "frame" => assert_eq!(step["n"], 0),
            "snap" => {
                let name = step["name"].as_str().unwrap();
                let want = golden(&file["screens"][name]);
                let got = cells(&panel.paint(70, 45, &f));
                for (y, (a, b)) in want.iter().zip(&got).enumerate() {
                    assert_eq!(a, b, "{name}: row {y}");
                }
                snaps += 1;
            }
            other => panic!("step {other}"),
        }
    }
    assert_eq!(snaps, 4);
}
