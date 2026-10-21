//! Fixtures and docs carry no real ids (Python tests/test_fixture_privacy.py, review M6): every
//! captured value and key is anonymised. A real tool-use id, agent id or session UUID (other
//! than the templates' all-zero one) in a top-level fixture, or a real agent id in the README
//! or a top-level doc, fails.

use regex::Regex;
use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const ZERO_UUID: &str = "00000000-0000-4000-8000-000000000000";

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == ext))
        .collect();
    out.sort();
    out
}

/// Python's `REAL_ID` matches in `text` (its UUID look-ahead as a filter).
fn real_ids(text: &str) -> Vec<String> {
    let re = Regex::new(r"toolu_[A-Za-z0-9]{20,}|\ba[0-9a-f]{16}\b|\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").unwrap();
    re.find_iter(text)
        .map(|m| m.as_str().to_owned())
        .filter(|m| m != ZERO_UUID)
        .collect()
}

#[test]
fn fixtures_hold_no_real_ids() {
    let found = files(&Path::new(ROOT).join("tests/fixtures"), "json");
    assert!(!found.is_empty());
    for p in found {
        let hits = real_ids(&std::fs::read_to_string(&p).unwrap());
        assert!(
            hits.is_empty(),
            "{}: {:?}",
            p.display(),
            &hits[..hits.len().min(3)]
        );
    }
}

#[test]
fn docs_hold_no_real_agent_ids() {
    let agent = Regex::new(r"\ba[0-9a-f]{16}\b").unwrap();
    let mut docs = vec![Path::new(ROOT).join("README.md")];
    docs.extend(files(&Path::new(ROOT).join("docs"), "md"));
    for p in docs {
        assert!(
            !agent.is_match(&std::fs::read_to_string(&p).unwrap()),
            "{}",
            p.display()
        );
    }
}

#[test]
fn the_rule_finds_what_it_is_for() {
    // the matcher itself: each kind of real id, and the templates' zero UUID passed over
    let text = format!(
        "toolu_{} a0123456789abcdef 1b4e28ba-2fa1-41d2-883f-0016d3cca427 {ZERO_UUID}",
        "A".repeat(24)
    );
    assert_eq!(real_ids(&text).len(), 3);
}
