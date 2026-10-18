//! The panel against its app-screen golden (`tests/fixtures/screens/panel.json`, captured
//! headless from Python's `PanelApp`): before the first state, shown, the core's filter, and
//! no core. The run's steps drive the panel's model as Python's capture drove the app.

use panorama::panel::model::Panel;
use panorama::panel::paint::Frame;
use panorama::proto::state::State;
use panorama_core::view::{PanelSettings, Screen, Tokens};
use serde_json::{Value, json};

fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A golden screen's cells, blanks showing only their background (tests/fixtures/README.md).
fn golden(runs: &Value) -> Vec<String> {
    let mut rows = vec![];
    for row in runs.as_array().unwrap() {
        let mut cells = String::new();
        for run in row.as_array().unwrap() {
            let (fg, bg) = (
                run[1].as_str().unwrap_or("#c3c7cd"),
                run[2].as_str().unwrap_or("#101215"),
            );
            let bold = run[3].as_bool().unwrap_or(false);
            for ch in run[0].as_str().unwrap().chars() {
                cells += &cell(ch, fg, bg, bold);
            }
        }
        rows.push(cells);
    }
    rows
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

fn cells(s: &Screen) -> Vec<String> {
    (0..s.h())
        .map(|y| {
            (0..s.w())
                .map(|x| {
                    let c = s.cell(x, y);
                    let bg = c.bg.map_or("#101215".to_owned(), |b| b.to_string());
                    cell(c.ch, &c.fg.to_string(), &bg, c.bold)
                })
                .collect()
        })
        .collect()
}

#[test]
fn the_panels_screens_are_pythons() {
    let file = fixture("screens/panel.json");
    let run = &file["runs"][0];
    let mut panel = Panel::new(
        run["pane_id"].as_str().map(Into::into),
        run["tab_id"].as_str().map(Into::into),
    );
    // no herdr config: herdr's default theme, panorama's fixed colours, the open key unbound
    let mut f = Frame {
        now: run["now"].as_f64().unwrap(),
        frame: 0.0,
        utc_offset: 0, // TZ=UTC
        theme: Tokens::builtin("catppuccin"),
        settings: PanelSettings::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let mut snaps = 0;
    for step in run["steps"].as_array().unwrap() {
        match step["do"].as_str().unwrap() {
            "core" => panel.on_connected(step["connected"].as_bool().unwrap()),
            "state" => {
                let swarm = fixture(step["swarm"].as_str().unwrap())["swarm"].clone();
                let mut st = json!({"swarm": swarm, "visible_tabs": step["visible"]});
                if let Some(name) = step.get("filter") {
                    st["filter"] = name.clone();
                }
                panel.on_state(State::read(st.as_object().unwrap()).unwrap());
            }
            "frame" => f.frame = step["n"].as_f64().unwrap(),
            "snap" => {
                let name = step["name"].as_str().unwrap();
                let (w, h) = (
                    run["size"][0].as_i64().unwrap() as i32,
                    run["size"][1].as_i64().unwrap() as i32,
                );
                let want = golden(&file["screens"][name]);
                let got = cells(&panel.paint(w, h, &f));
                for (y, (a, b)) in want.iter().zip(&got).enumerate() {
                    assert_eq!(a, b, "{name}: row {y} differs");
                }
                assert_eq!(want.len(), got.len(), "{name}: rows");
                snaps += 1;
            }
            other => panic!("step {other}"),
        }
    }
    assert_eq!(snaps, 4);
}
