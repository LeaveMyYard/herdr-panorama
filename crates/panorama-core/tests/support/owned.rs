//! Rust-owned screen goldens (plan `uniform-agent-tree.md`, D5): the design references and
//! Python captures encode threads, so since the uniform tree these screens are Rust's own,
//! `tests/fixtures/rust/screens/<file>`, written by `PANORAMA_BLESS=1` and reviewed as a diff.

use super::{assert_same, cells, fixture, golden};
use panorama_core::view::Screen;
use serde_json::{Map, Value, json};
use std::sync::Mutex;

/// One test binary's writes to a file, one at a time.
static WRITING: Mutex<()> = Mutex::new(());

/// Whether this run rewrites the Rust-owned goldens instead of comparing against them.
pub fn blessing() -> bool {
    std::env::var_os("PANORAMA_BLESS").is_some_and(|v| v == "1")
}

/// `got` against screen `name` of `rust/screens/<file>`; blessing, writes it there.
pub fn check_owned(file: &str, name: &str, got: &Screen) {
    let rel = format!("rust/screens/{file}");
    if !blessing() {
        let want = fixture(&rel);
        let screen = &want["screens"][name];
        assert!(!screen.is_null(), "{rel}: no screen {name}");
        return assert_same(&format!("{file} {name}"), &golden(screen), &cells(got));
    }
    let _guard = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let path = format!("{}/../../tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    let mut doc: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "screens": {} }));
    doc["source"] = json!(
        "Rust's own screens, written by PANORAMA_BLESS=1 cargo test (plan uniform-agent-tree.md, \
         D5)"
    );
    let screens = doc["screens"].as_object_mut().unwrap();
    screens.insert(name.to_owned(), runs(got));
    let sorted: Map<String, Value> = std::mem::take(screens).into_iter().collect();
    doc["screens"] = Value::Object(sorted);
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

/// A screen as golden rows of `[text, fg, bg, bold]` runs (the captures' form).
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
                .map(|(t, fg, bg, bold)| json!([t, fg, bg, bold]))
                .collect(),
        )
    });
    Value::Array(rows.collect())
}
