use super::*;
use crate::machines::tests::SID;
use std::os::unix::fs::symlink;

const LIVE: &str = "aaaaaaaa-0000-0000-0000-000000000001";
const OLD: &str = "bbbbbbbb-0000-0000-0000-000000000002";
const CONT_OLD: &str = "77777777-7777-4777-8777-777777777777";
const CONT_NEW: &str = "88888888-8888-4888-8888-888888888888";

fn scratch(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(name).tempdir().unwrap()
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn prune_drops_sessions_gone_longer_than_the_done_window() {
    let dir = scratch("prune-window");
    let dest = dir.path().join("mirror");
    fs::create_dir_all(dest.join("proj").join(SID).join("subagents")).unwrap();
    fs::write(dest.join("proj").join(format!("{SID}.jsonl")), "a").unwrap();
    fs::write(
        dest.join("proj").join(SID).join("subagents/agent-x.jsonl"),
        "x",
    )
    .unwrap();
    assert!(prune(&dest, &ids(&[SID]), 3600.0, 1000.0).is_empty()); // named now
    assert!(prune(&dest, &[], 3600.0, 4000.0).is_empty()); // gone, within the window
    assert_eq!(prune(&dest, &[], 3600.0, 5000.0), [SID]);
    assert!(!dest.join("proj").join(format!("{SID}.jsonl")).exists());
    assert!(!dest.join("proj").join(SID).exists());
}

#[test]
fn prune_never_touches_live_sessions_or_leaves_the_project() {
    let dir = scratch("prune-probe");
    let (dest, outside) = (dir.path().join("mirror"), dir.path().join("outside"));
    let proj = dest.join("-p1");
    for sid in [LIVE, OLD] {
        fs::create_dir_all(proj.join(sid)).unwrap();
        fs::write(proj.join(format!("{sid}.jsonl")), "x").unwrap();
    }
    fs::write(proj.join("...jsonl"), "x").unwrap(); // stem ".."
    fs::write(proj.join(".jsonl"), "x").unwrap(); // stem ""
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), "mine").unwrap();
    symlink(&outside, proj.join("evil")).unwrap();
    fs::write(proj.join("evil.jsonl"), "x").unwrap();
    prune(&dest, &ids(&[LIVE, OLD]), 3600.0, 0.0);
    let gone = prune(&dest, &ids(&[LIVE]), 3600.0, 7200.0);
    assert_eq!(gone, [OLD]); // only a well-formed, stale session id
    assert!(proj.join(format!("{LIVE}.jsonl")).exists() && proj.join(LIVE).is_dir());
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "mine");
    assert!(proj.join("evil").symlink_metadata().unwrap().is_symlink());
    assert!(!proj.join(format!("{OLD}.jsonl")).exists() && !proj.join(OLD).exists());
}

#[test]
fn a_symlinked_session_dir_or_transcript_is_never_deleted() {
    let dir = scratch("prune-links");
    let (dest, outside) = (dir.path().join("mirror"), dir.path().join("outside"));
    let proj = dest.join("p");
    fs::create_dir_all(&proj).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("t.jsonl"), "keep").unwrap();
    symlink(outside.join("t.jsonl"), proj.join(format!("{OLD}.jsonl"))).unwrap();
    fs::write(proj.join(format!("{LIVE}.jsonl")), "x").unwrap();
    symlink(&outside, proj.join(LIVE)).unwrap();
    prune(&dest, &ids(&[OLD, LIVE]), 60.0, 0.0);
    assert!(prune(&dest, &[], 60.0, 1000.0).is_empty());
    assert_eq!(fs::read_to_string(outside.join("t.jsonl")).unwrap(), "keep");
    assert!(proj.join(format!("{LIVE}.jsonl")).exists());
}

#[test]
fn prune_keeps_a_continuation_while_its_old_session_is_named() {
    let dir = scratch("prune-continued");
    let dest = dir.path().join("mirror");
    fs::create_dir_all(dest.join("proj")).unwrap();
    for sid in [CONT_OLD, CONT_NEW] {
        fs::write(dest.join("proj").join(format!("{sid}.jsonl")), "x\n").unwrap();
    }
    prune(&dest, &ids(&[CONT_OLD, CONT_NEW]), 60.0, 1000.0);
    prune(&dest, &ids(&[CONT_OLD, CONT_NEW]), 60.0, 5000.0); // still named through the old one
    assert!(dest.join("proj").join(format!("{CONT_NEW}.jsonl")).exists());
}

#[test]
fn a_transcript_mirrored_before_the_bookkeeping_starts_its_clock_now() {
    let dir = scratch("prune-clock");
    let dest = dir.path().join("mirror");
    fs::create_dir_all(dest.join("p")).unwrap();
    fs::write(dest.join("p").join(format!("{SID}.jsonl")), "x").unwrap();
    fs::write(dest.join(SEEN), r#"{"bad": 1, "x": "y"}"#).unwrap();
    assert!(prune(&dest, &[], 60.0, 1000.0).is_empty());
    let seen: Value = serde_json::from_str(&fs::read_to_string(dest.join(SEEN)).unwrap()).unwrap();
    assert_eq!(seen, serde_json::json!({ SID: 1000.0 }));
    assert_eq!(prune(&dest, &[], 60.0, 1061.0), [SID]);
}

#[test]
fn a_broken_seen_file_starts_over_and_no_mirror_is_fine() {
    let dir = scratch("prune-broken");
    let dest = dir.path().join("mirror");
    assert!(prune(&dest, &ids(&[SID]), 60.0, 0.0).is_empty()); // no mirror dir yet
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join(SEEN), "[1, 2").unwrap();
    assert!(prune(&dest, &ids(&[SID]), 60.0, 0.0).is_empty());
    assert!(fs::read_to_string(dest.join(SEEN)).unwrap().contains(SID));
}
