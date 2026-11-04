//! Reading the app-screen goldens (tests/fixtures/README.md, "Screens") and a rendered screen
//! in the same shape: one string per cell, a blank showing only its background.

#![allow(dead_code)] // each test binary uses its own part

pub mod player;

#[path = "../harness_goldens/mod.rs"]
mod harness_goldens;

use panorama_core::view::Screen;
use serde_json::Value;

pub fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    harness_goldens::adapt(
        rel,
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap(),
    )
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

/// Whether this run rewrites the Rust-owned goldens (`tests/fixtures/rust/`) instead of
/// comparing against them: `PANORAMA_BLESS=1`. The rewrite is then reviewed as a diff.
pub fn blessing() -> bool {
    std::env::var_os("PANORAMA_BLESS").is_some_and(|v| v == "1")
}

/// A rendered screen as golden runs of `[text, fg, bg, bold]` per row (what [`golden`] reads):
/// a blank shows only its background, so it joins any run of its background.
pub fn runs(s: &Screen) -> Value {
    let rows = (0..s.h()).map(|y| {
        let mut row: Vec<(String, Option<String>, String, bool)> = vec![];
        for x in 0..s.w() {
            let c = s.cell(x, y);
            let bg =
                c.bg.map_or("#101215".to_owned(), |b| b.to_string().to_lowercase());
            let fg = (c.ch != ' ').then(|| c.fg.to_string().to_lowercase());
            let bold = c.ch != ' ' && c.bold;
            match row.last_mut() {
                Some(last)
                    if last.2 == bg
                        && (fg.is_none()
                            || last.1.is_none()
                            || (last.1 == fg && last.3 == bold)) =>
                {
                    last.0.push(c.ch);
                    if last.1.is_none() && fg.is_some() {
                        (last.1, last.3) = (fg, bold);
                    }
                }
                _ => row.push((c.ch.to_string(), fg, bg, bold)),
            }
        }
        Value::Array(
            row.into_iter()
                .map(|(t, fg, bg, bold)| serde_json::json!([t, fg, bg, bold]))
                .collect(),
        )
    });
    Value::Array(rows.collect())
}
