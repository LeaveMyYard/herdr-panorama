//! No test may reach a live herdr (Python's tests/test_panel_sync.py
//! `test_tests_never_reach_a_live_herdr`, the 10-17 leak class): every test that starts the
//! `panorama` binary clears its environment, so no `HERDR_*` of the session the suite runs in
//! (its socket, its pane) reaches the child. Library tests build their herdr from a fake socket
//! (`FakeHerdr`), never `Herdr::from_env()`.

use std::path::Path;

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_test_that_starts_the_binary_clears_its_environment() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    for d in ["tests", "src"] {
        rust_files(&root.join(d), &mut files);
    }
    let this = Path::new(file!()).file_name().unwrap();
    let mut spawners = 0;
    for f in files.iter().filter(|f| f.file_name() != Some(this)) {
        let text = std::fs::read_to_string(f).unwrap();
        if text.contains("CARGO_BIN_EXE_panorama") {
            spawners += 1;
            assert!(
                text.contains(".env_clear()"),
                "{} starts panorama without env_clear()",
                f.display()
            );
        }
        let is_test = f.components().any(|c| c.as_os_str() == "tests") || f.ends_with("tests.rs");
        if is_test {
            assert!(
                !text.contains("Herdr::from_env()"),
                "{} builds a herdr from the environment",
                f.display()
            );
        }
    }
    assert!(
        spawners > 0,
        "no test starts the binary: this check checks nothing"
    );
}
