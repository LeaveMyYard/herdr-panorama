//! Reading the app-screen goldens (tests/fixtures/README.md, "Screens") and a rendered screen
//! in the same shape: one string per cell, a blank showing only its background.

#![allow(dead_code)] // each test binary uses its own part

pub mod player;

use panorama_core::view::Screen;
use serde_json::Value;

pub fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A golden screen's cells, blanks showing only their background (tests/fixtures/README.md).
pub fn golden(runs: &Value) -> Vec<String> {
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

pub fn cells(s: &Screen) -> Vec<String> {
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
