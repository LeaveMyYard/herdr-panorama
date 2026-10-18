use super::*;
use crate::machines::tests::SID;
use std::fs;
use std::path::PathBuf;

fn scratch(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(name).tempdir().unwrap()
}

/// A session as Claude Code lays it out, plus files the scanner never reads
/// (`test_machines.remote_tree`).
fn remote_tree(root: &Path) {
    let s = root.join("proj").join(SID);
    for d in ["subagents/workflows/run1", "workflows", "tool-results"] {
        fs::create_dir_all(s.join(d)).unwrap();
    }
    let files = [
        (format!("proj/{SID}.jsonl"), "a\n"),
        (format!("proj/{SID}/subagents/agent-x.jsonl"), "x\n"),
        (
            format!("proj/{SID}/subagents/agent-x.meta.json"),
            r#"{"status": "running"}"#,
        ),
        (
            format!("proj/{SID}/subagents/workflows/run1/journal.jsonl"),
            "j\n",
        ),
        (
            format!("proj/{SID}/subagents/workflows/run1/agent-y.jsonl"),
            "y\n",
        ),
        (
            format!("proj/{SID}/subagents/workflows/run1/agent-y.meta.json"),
            "{}",
        ),
        (
            format!("proj/{SID}/workflows/run1.json"),
            r#"{"status": "running"}"#,
        ),
        (format!("proj/{SID}/tool-results/big.txt"), "never read"),
        (format!("proj/{SID}/notes.md"), "never read"),
        (
            "proj/99999999-0000-0000-0000-000000000000.jsonl".to_owned(),
            "another session",
        ),
    ];
    for (rel, text) in files {
        fs::write(root.join(rel), text).unwrap();
    }
}

fn mirrored(dest: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if p.file_name().unwrap() != "seen.json" {
                out.push(p.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dest, dest, &mut out);
    out.sort();
    out
}

fn local(src: &Path, dest: &Path) -> (String, PathBuf) {
    (format!("{}/", src.display()), dest.to_path_buf())
}

fn mirror_now(rsync: &Rsync, src: &Path, dest: &Path) -> Option<String> {
    let (source, dest) = local(src, dest);
    rsync.mirror(&source, &dest, &[SID.to_owned()], run)
}

#[test]
fn mirror_copies_only_what_the_scanner_reads() {
    let dir = scratch("mirror-only");
    let (src, dest) = (dir.path().join("remote"), dir.path().join("mirror"));
    remote_tree(&src);
    fs::create_dir(&dest).unwrap();
    assert_eq!(mirror_now(&Rsync::new(None), &src, &dest), None);
    let mut want: Vec<String> = [
        format!("proj/{SID}.jsonl"),
        format!("proj/{SID}/subagents/agent-x.jsonl"),
        format!("proj/{SID}/subagents/agent-x.meta.json"),
        format!("proj/{SID}/subagents/workflows/run1/agent-y.jsonl"),
        format!("proj/{SID}/subagents/workflows/run1/agent-y.meta.json"),
        format!("proj/{SID}/subagents/workflows/run1/journal.jsonl"),
        format!("proj/{SID}/workflows/run1.json"),
    ]
    .to_vec();
    want.sort();
    assert_eq!(mirrored(&dest), want);
}

#[test]
fn mirror_follows_appends_and_in_place_rewrites() {
    let dir = scratch("mirror-rewrites");
    let (src, dest) = (dir.path().join("remote"), dir.path().join("mirror"));
    remote_tree(&src);
    fs::create_dir(&dest).unwrap();
    let rsync = Rsync::new(None);
    mirror_now(&rsync, &src, &dest);
    let session = src.join("proj").join(SID);
    fs::write(src.join("proj").join(format!("{SID}.jsonl")), "a\nb\n").unwrap(); // appended
    fs::write(session.join("subagents/agent-x.meta.json"), "{}").unwrap(); // shorter
    let summary = session.join("workflows/run1.json");
    fs::write(&summary, r#"{"status": "done!!"}"#).unwrap(); // the same size
    let later = fs::metadata(&summary).unwrap().modified().unwrap() + Duration::from_secs(5);
    fs::File::options()
        .write(true)
        .open(&summary)
        .unwrap()
        .set_modified(later)
        .unwrap();
    mirror_now(&rsync, &src, &dest);
    let got = |rel: &str| fs::read_to_string(dest.join("proj").join(rel)).unwrap();
    assert_eq!(got(&format!("{SID}.jsonl")), "a\nb\n");
    assert_eq!(got(&format!("{SID}/subagents/agent-x.meta.json")), "{}");
    assert_eq!(
        got(&format!("{SID}/workflows/run1.json")),
        r#"{"status": "done!!"}"#
    );
}

#[test]
fn mirror_caps_the_file_size() {
    let dir = scratch("mirror-cap");
    let (src, dest) = (dir.path().join("remote"), dir.path().join("mirror"));
    remote_tree(&src);
    fs::write(
        src.join("proj").join(format!("{SID}.jsonl")),
        "x".repeat(5000),
    )
    .unwrap();
    fs::create_dir(&dest).unwrap();
    let rsync = Rsync {
        max_file: "1K".into(),
        ..Rsync::new(None)
    };
    mirror_now(&rsync, &src, &dest);
    assert!(!dest.join("proj").join(format!("{SID}.jsonl")).exists());
    assert!(
        dest.join("proj")
            .join(SID)
            .join("subagents/agent-x.jsonl")
            .exists()
    );
}

#[test]
fn the_first_failed_pass_stops_the_mirror() {
    let calls = std::sync::Mutex::new(0);
    let failing = |_: &[String], _: Duration| {
        *calls.lock().unwrap() += 1;
        Some("no route".to_owned())
    };
    let err = Rsync::new(None).mirror("h:x/", Path::new("/m"), &[SID.to_owned()], failing);
    assert_eq!(err.as_deref(), Some("no route"));
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn a_mirror_deletes_nothing_already_in_the_dest() {
    const OTHER: &str = "bbbbbbbb-0000-0000-0000-000000000002";
    let dir = scratch("mirror-keeps");
    let (src, dest) = (dir.path().join("remote"), dir.path().join("mirror"));
    fs::create_dir_all(src.join("p")).unwrap();
    fs::write(src.join("p").join(format!("{SID}.jsonl")), "a\n").unwrap();
    fs::create_dir_all(dest.join("p")).unwrap();
    fs::write(dest.join("p").join(format!("{OTHER}.jsonl")), "mine").unwrap();
    fs::write(dest.join("unrelated.txt"), "mine").unwrap();
    let source = format!("{}/", src.display());
    let named = [SID.to_owned(), OTHER.to_owned()];
    assert_eq!(Rsync::new(None).mirror(&source, &dest, &named, run), None);
    assert_eq!(
        fs::read_to_string(dest.join("p").join(format!("{OTHER}.jsonl"))).unwrap(),
        "mine"
    );
    assert_eq!(
        fs::read_to_string(dest.join("unrelated.txt")).unwrap(),
        "mine"
    );
    assert_eq!(
        fs::read_to_string(dest.join("p").join(format!("{SID}.jsonl"))).unwrap(),
        "a\n"
    );
}

#[test]
fn building_an_rsync_never_runs_it() {
    assert_eq!(Rsync::new(None).append, None); // asked on first use, on a watcher's thread
    let off = Rsync {
        append: Some(false),
        ..Rsync::new(None)
    };
    let [jsonl, _] = off.passes("h:x/", Path::new("/m"), &[SID.to_owned()]);
    assert!(!jsonl.iter().any(|a| a == "--append-verify"));
    let on = Rsync {
        append: Some(true),
        ..Rsync::new(None)
    };
    let [jsonl, json] = on.passes("h:x/", Path::new("/m"), &[SID.to_owned()]);
    assert!(
        jsonl.iter().any(|a| a == "--append-verify")
            && !json.iter().any(|a| a == "--append-verify")
    );
}
