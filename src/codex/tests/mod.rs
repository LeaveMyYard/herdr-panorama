mod cache;
mod discovery;
mod history;
mod parse;
use super::Options;
use std::{fs, path::PathBuf, time::Duration};

fn fixture(mode: &str) -> (tempfile::TempDir, Options) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mode"), mode).unwrap();
    let executable = dir.path().join("fake-codex");
    fs::write(&executable, include_str!("fixture.py")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let options = Options {
        executable,
        codex_home: PathBuf::from(dir.path()),
        timeout: Duration::from_secs(2),
        refresh_interval: Duration::from_secs(60),
    };
    (dir, options)
}
fn calls(dir: &std::path::Path) -> Vec<serde_json::Value> {
    fs::read_to_string(dir.join("calls"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
