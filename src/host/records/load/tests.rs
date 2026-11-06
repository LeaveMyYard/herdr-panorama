use super::*;
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};

const A: &str = "11111111-1111-4111-8111-111111111111";
const B: &str = "22222222-2222-4222-8222-222222222222";
const C: &str = "33333333-3333-4333-8333-333333333333";

fn session_line() -> String {
    json!({"v":1,"event":"session","host":"herdr-core","terminal":"t2","harness":"claude",
        "pid":20,"pstart":"boot-1","ts":1.0})
    .to_string()
        + "\n"
}

/// A hook file holding one session record, padded to `size`, last written at `age`.
fn hook_file(dir: &Path, session: &str, size: usize, age: u64) {
    let mut text = session_line();
    text.push_str(&"x".repeat(size - text.len()));
    let path = dir.join(format!("{session}.jsonl"));
    std::fs::write(&path, text).unwrap();
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000 - age))
        .unwrap();
}

fn sessions(hooks: &Hooks) -> Vec<&str> {
    hooks.records.iter().map(|r| r.session.as_str()).collect()
}

fn limits(file: u64, total: u64) -> Limits {
    Limits {
        entries: 100,
        file,
        total,
    }
}

#[test]
fn a_record_is_found_wherever_it_sits_in_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let noise = "{\"v\":1,\"event\":\"stop\",\"agent\":\"session\"}\n".repeat(5000);
    let unterminated = session_line().trim_end().to_owned();
    for (session, text) in [
        (A, format!("{}{noise}", session_line())),
        (B, format!("{noise}{}{noise}", session_line())),
        (C, format!("{noise}{unterminated}")),
    ] {
        std::fs::write(dir.path().join(format!("{session}.jsonl")), text).unwrap();
    }
    let hooks = load(dir.path());
    assert!(hooks.complete);
    let mut found = sessions(&hooks);
    found.sort_unstable();
    assert_eq!(found, [A, B, C]);
}

#[test]
fn the_budget_goes_to_the_newest_files_whatever_the_directory_order() {
    for (newest, oldest) in [(A, B), (B, A)] {
        let dir = tempfile::tempdir().unwrap();
        hook_file(dir.path(), newest, 400, 1);
        hook_file(dir.path(), oldest, 400, 2);
        let hooks = scan(dir.path(), &limits(1000, 500));
        assert_eq!(sessions(&hooks), [newest]);
        assert!(!hooks.complete);
    }
}

#[test]
fn equal_times_are_read_in_name_order() {
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), B, 400, 1);
    hook_file(dir.path(), A, 400, 1);
    assert_eq!(sessions(&scan(dir.path(), &limits(1000, 500))), [A]);
}

#[test]
fn an_oversized_file_is_refused_and_costs_no_budget() {
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 900, 1);
    hook_file(dir.path(), B, 400, 2);
    let hooks = scan(dir.path(), &limits(500, 500));
    assert_eq!(sessions(&hooks), [B]);
    assert!(!hooks.complete);
}

#[test]
fn the_limits_are_inclusive_and_a_smaller_older_file_still_fits() {
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 400, 1);
    hook_file(dir.path(), B, 400, 2);
    let exact = scan(dir.path(), &limits(400, 800));
    assert_eq!(sessions(&exact), [A, B]);
    assert!(exact.complete);
    let short = scan(dir.path(), &limits(400, 799));
    assert_eq!(sessions(&short), [A]);
    assert!(!short.complete);
    hook_file(dir.path(), C, 300, 3);
    let skipped = scan(dir.path(), &limits(400, 700));
    assert_eq!(sessions(&skipped), [A, C]);
    assert!(!skipped.complete);
}

#[test]
fn too_many_entries_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 400, 1);
    hook_file(dir.path(), B, 400, 2);
    let at = |entries| {
        let limits = Limits {
            entries,
            file: 1000,
            total: 1000,
        };
        scan(dir.path(), &limits)
    };
    assert!(at(2).complete);
    assert_eq!(at(2).records.len(), 2);
    let cut = at(1);
    assert!(!cut.complete);
    assert_eq!(cut.records.len(), 1);
}

#[test]
fn only_a_hook_file_that_exists_and_stays_unread_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load(&dir.path().join("no-hooks")).complete);
    // Not hook files: nothing they hold could be evidence.
    std::fs::write(dir.path().join("notes.txt"), session_line()).unwrap();
    std::fs::write(dir.path().join("not-a-session!.jsonl"), session_line()).unwrap();
    std::fs::create_dir(dir.path().join(format!("{C}.jsonl"))).unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("notes.txt"),
        dir.path().join(format!("{B}.jsonl")),
    )
    .unwrap();
    let hooks = load(dir.path());
    assert!(hooks.complete);
    assert!(hooks.records.is_empty());
    // A file pruned after it was listed left nothing to read.
    let mut bytes = Vec::new();
    assert!(read(&dir.path().join("gone.jsonl"), 10, &mut bytes));
    // One that cannot be opened (a directory reads as an error) may hold anything.
    assert!(!read(
        &dir.path().join(format!("{C}.jsonl")),
        10,
        &mut bytes
    ));
}

#[test]
fn a_listed_file_that_cannot_be_read_leaves_the_load_incomplete() {
    use std::os::unix::fs::PermissionsExt;
    // Root opens any file, so the refusal below cannot be staged for it.
    if rustix::process::getuid().is_root() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 400, 1);
    hook_file(dir.path(), B, 400, 2);
    let locked = dir.path().join(format!("{A}.jsonl"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let hooks = load(dir.path());
    assert_eq!(sessions(&hooks), [B]);
    assert!(!hooks.complete);
}

#[test]
fn a_directory_that_cannot_be_listed_or_inspected_leaves_the_load_incomplete() {
    use std::os::unix::fs::PermissionsExt;
    // Root reads any directory, so the refusals below cannot be staged for it.
    if rustix::process::getuid().is_root() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 400, 1);
    let mode = |bits| {
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(bits)).unwrap();
    };
    // 0o000: the listing fails. 0o400: names are listed, but no file can be inspected.
    mode(0o000);
    let unlisted = load(dir.path());
    mode(0o400);
    let uninspected = load(dir.path());
    mode(0o700);
    assert!(!unlisted.complete);
    assert!(!uninspected.complete);
    assert!(load(dir.path()).complete);
}

#[test]
fn a_file_longer_than_it_was_listed_stops_at_the_limit_and_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    hook_file(dir.path(), A, 400, 1);
    let path = dir.path().join(format!("{A}.jsonl"));
    let mut bytes = Vec::new();
    assert!(!read(&path, 399, &mut bytes));
    assert_eq!(bytes.len(), 400);
    bytes.clear();
    assert!(read(&path, 400, &mut bytes));
}

#[test]
fn a_line_needs_every_field_of_a_core_session_record() {
    let good = session_line();
    assert!(parse(A, good.trim_end().as_bytes()).is_some());
    for broken in [
        "{\"event\":\"session\"",
        "{\"v\":1,\"event\":\"start\",\"agent\":\"session\"}",
        "{\"v\":1,\"event\":\"session\",\"host\":\"herdr-core\"}",
    ] {
        assert!(parse(A, broken.as_bytes()).is_none(), "{broken}");
    }
}
