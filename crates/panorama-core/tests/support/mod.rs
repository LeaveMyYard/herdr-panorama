//! Reading the captured goldens (tests/fixtures/README.md) and comparing screens cell by
//! cell, as the README says a port compares them.

#![allow(dead_code)] // each test binary uses its own part

pub mod design;
pub mod glyphs;

/// The no-sessions words the Claude Code panel passes (Python's).
pub const NO_SESSIONS: &str = "No Claude Code sessions in herdr";

use panorama_core::model::Swarm;
use panorama_core::view::Screen;
use panorama_core::view::palette::{BG, FG};
use serde_json::Value;
use std::path::PathBuf;

/// A fixture under tests/fixtures/, as JSON.
pub fn fixture(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// A captured swarm in wire form (`inputs/swarm-*.json`), docked next to `docked`.
pub fn wire_swarm(rel: &str, docked: Option<&str>) -> Swarm {
    let mut sw: Swarm = serde_json::from_value(fixture(rel)["swarm"].clone()).unwrap();
    sw.docked = docked.map(Into::into);
    sw
}

/// One cell as the goldens compare it: a blank shows only its background (its colour and
/// weight are invisible); no colour means the default (`FG`, `BG`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub ch: char,
    pub fg: Option<String>,
    pub bg: String,
    pub bold: Option<bool>,
}

fn shown(ch: char, fg: Option<&str>, bg: Option<&str>, bold: bool) -> Shown {
    let bg = bg.map_or_else(|| BG.to_string(), str::to_lowercase);
    if ch == ' ' {
        return Shown {
            ch,
            fg: None,
            bg,
            bold: None,
        };
    }
    Shown {
        ch,
        fg: Some(fg.map_or_else(|| FG.to_string(), str::to_lowercase)),
        bg,
        bold: Some(bold),
    }
}

/// A golden screen (rows of `[text, fg, bg, bold]` runs) as cells.
pub fn golden(runs: &Value) -> Vec<Vec<Shown>> {
    runs.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .flat_map(|run| {
                    let (fg, bg) = (run[1].as_str(), run[2].as_str());
                    let bold = run[3].as_bool().unwrap_or(false);
                    run[0]
                        .as_str()
                        .unwrap()
                        .chars()
                        .map(move |ch| shown(ch, fg, bg, bold))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}

/// A rendered screen as cells.
pub fn cells(s: &Screen) -> Vec<Vec<Shown>> {
    (0..s.h())
        .map(|y| {
            (0..s.w())
                .map(|x| {
                    let c = s.cell(x, y);
                    shown(
                        c.ch,
                        Some(&c.fg.to_string()),
                        c.bg.map(|b| b.to_string()).as_deref(),
                        c.bold,
                    )
                })
                .collect()
        })
        .collect()
}

/// The cells that differ, `(x, y, want, got)`; a size mismatch is one difference.
pub fn diff(want: &[Vec<Shown>], got: &[Vec<Shown>]) -> Vec<String> {
    if want.len() != got.len() || want.iter().zip(got).any(|(a, b)| a.len() != b.len()) {
        return vec![format!("size: want {} rows, got {}", want.len(), got.len())];
    }
    let mut out = vec![];
    for (y, (a, b)) in want.iter().zip(got).enumerate() {
        for (x, (ca, cb)) in a.iter().zip(b).enumerate() {
            if ca != cb {
                let text = |r: &[Shown]| r.iter().map(|c| c.ch).collect::<String>();
                out.push(format!(
                    "({x},{y}) want {ca:?} got {cb:?}\n  want row {:?}\n  got  row {:?}",
                    text(a),
                    text(b)
                ));
            }
        }
    }
    out
}

/// Fails with the first differences.
pub fn assert_same(name: &str, want: &[Vec<Shown>], got: &[Vec<Shown>]) {
    let d = diff(want, got);
    assert!(
        d.is_empty(),
        "{name}: {} cells differ, first:\n{}",
        d.len(),
        d[..d.len().min(3)].join("\n")
    );
}
