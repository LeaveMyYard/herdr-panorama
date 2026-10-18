//! The app's frames against the app-screen goldens captured from Python's `SwarmApp`
//! (`screens/demo-wide.json`, `screens/dock.json`), from its model as a refresh leaves it.

use super::*;
use crate::ui::frame::Frame;
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, Screen};
use serde_json::Value;
use std::time::Instant;

fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A run's frame: its clock, frame 0, UTC, panorama's own colours.
fn frame(now: f64) -> Frame {
    Frame {
        now,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    }
}

/// The screen as `(char, fg, bg, bold)` per cell, blanks by their background only.
fn cells(s: &Screen) -> Vec<Vec<(char, String, String, bool)>> {
    let bg = |c: Option<panorama_core::view::Rgb>| {
        c.unwrap_or(panorama_core::view::palette::BG).to_string()
    };
    (0..s.h())
        .map(|y| {
            (0..s.w())
                .map(|x| {
                    let c = s.cell(x, y);
                    if c.ch == ' ' {
                        (' ', String::new(), bg(c.bg), false)
                    } else {
                        (c.ch, c.fg.to_string(), bg(c.bg), c.bold)
                    }
                })
                .collect()
        })
        .collect()
}

fn golden(runs: &Value) -> Vec<Vec<(char, String, String, bool)>> {
    use panorama_core::view::palette::{BG, FG};
    runs.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .flat_map(|r| {
                    let fg = r[1].as_str().map_or(FG.to_string(), str::to_lowercase);
                    let bg = r[2].as_str().map_or(BG.to_string(), str::to_lowercase);
                    let bold = r[3].as_bool().unwrap_or(false);
                    r[0].as_str()
                        .unwrap()
                        .chars()
                        .map(move |ch| {
                            if ch == ' ' {
                                (' ', String::new(), bg.clone(), false)
                            } else {
                                (ch, fg.clone(), bg.clone(), bold)
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}

/// The app of `file`'s first run after its first refresh, and that run.
fn app_of(file: &str) -> (App, Value) {
    let f = fixture(&format!("screens/{file}"));
    let run = f["runs"][0].clone();
    let input = fixture(run["input"].as_str().unwrap());
    let mut swarm: Swarm = serde_json::from_value(input["swarm"].clone()).unwrap();
    swarm.docked = run["docked"].as_str().map(Into::into);
    let home = run["home_mode"].as_bool().unwrap();
    let mut app = App::new(
        run["focus_pane"].as_str().map(Into::into),
        home,
        Instant::now(),
    );
    app.apply(swarm, Marks::new(), Instant::now());
    (app, f)
}

fn check(file: &str, snap: &str) {
    let (mut app, f) = app_of(file);
    let run = &f["runs"][0];
    let (w, h) = (
        run["size"][0].as_i64().unwrap() as i32,
        run["size"][1].as_i64().unwrap() as i32,
    );
    let s = app
        .paint(w, h, &frame(run["now"].as_f64().unwrap()))
        .unwrap();
    let (want, got) = (golden(&f["screens"][snap]), cells(&s));
    let diffs: Vec<String> = (0..want.len())
        .flat_map(|y| (0..want[y].len()).map(move |x| (x, y)))
        .filter(|&(x, y)| want[y][x] != got[y][x])
        .take(3)
        .map(|(x, y)| format!("({x},{y}) want {:?} got {:?}", want[y][x], got[y][x]))
        .collect();
    assert!(diffs.is_empty(), "{file} {snap}:\n{}", diffs.join("\n"));
}

#[test]
fn the_popups_first_frame_is_pythons() {
    check("demo-wide.json", "initial");
}

#[test]
fn the_home_tabs_docked_first_frame_is_pythons() {
    check("dock.json", "focused");
}

#[test]
fn the_narrow_first_frame_is_pythons() {
    check("demo-narrow.json", "initial");
}

#[test]
fn a_failed_refresh_says_so_in_the_header_and_no_size_paints_nothing() {
    let (mut app, _) = app_of("demo-wide.json");
    app.failed("RuntimeError: bad record");
    let s = app.paint(160, 45, &frame(0.0)).unwrap();
    assert!(
        s.text()
            .lines()
            .next()
            .unwrap()
            .contains(" refresh failed  RuntimeError: bad record")
    );
    assert!(app.paint(0, 45, &frame(0.0)).is_none());
}

#[test]
fn a_sideways_scroll_is_clamped_to_the_first_column_shown() {
    let (mut app, _) = app_of("deep.json");
    app.view.thread = Some("x:deep".into());
    app.view.agent = Some("c1".into());
    app.view.focus = 5;
    app.view.hcol = Some(99);
    app.paint(160, 45, &frame(0.0)).unwrap();
    assert_eq!(app.view.hcol, Some(6));
}
