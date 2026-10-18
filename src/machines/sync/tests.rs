use super::*;
use crate::machines::tests::SID;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

type Calls = Arc<Mutex<Vec<Vec<String>>>>;

fn env(state: &Path) -> PluginEnv {
    PluginEnv::from_vars(
        [
            ("HERDR_PLUGIN_STATE_DIR", state.as_os_str()),
            ("XDG_RUNTIME_DIR", std::ffi::OsStr::new("/run/me")),
        ],
        None,
        1000,
    )
}

/// A sync whose rsync runs are recorded, not run (Python's `_run` monkeypatched).
fn recording(state: &Path, append: bool) -> (RsyncSync, Calls) {
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    let mut s = RsyncSync::new(&env(state), Arc::new(|| 3600.0));
    s.rsync.append = append;
    s.runner = Arc::new(move |argv: &[String], _| {
        seen.lock().unwrap().push(argv.to_vec());
        None
    });
    (s, calls)
}

fn devbox() -> Machine {
    Machine::new("abc123", "devbox", "devbox")
}

fn scratch(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(name).tempdir().unwrap()
}

#[test]
fn sync_mirrors_over_batch_ssh_in_two_passes() {
    let dir = scratch("sync-passes");
    let state = dir.path().join("state");
    let (s, calls) = recording(&state, true);
    assert_eq!(s.sync(&devbox(), &[SID.to_owned()]), None);
    let seen = calls.lock().unwrap().clone();
    assert_eq!(seen.len(), 2); // .jsonl append-only, then .json normally
    assert!(seen[0].iter().any(|a| a == "--append-verify"));
    assert!(!seen[1].iter().any(|a| a == "--append-verify"));
    for argv in &seen {
        let n = argv.len();
        assert_eq!(argv[0], "rsync");
        assert_eq!(argv[n - 2], "devbox:.claude/projects/");
        assert!(argv.iter().any(|a| a == "--max-size=64M"));
        let ssh = &argv[argv.iter().position(|a| a == "-e").unwrap() + 1];
        for opt in ["BatchMode=yes", "ConnectTimeout=5", "ControlMaster=auto"] {
            assert!(ssh.contains(opt), "{ssh}");
        }
        assert!(ssh.contains("ControlPath=/run/me/panorama-ssh-%C"), "{ssh}");
        assert!(argv[n - 1].starts_with(&state.join("remote").display().to_string()));
    }
    assert_eq!(s.sync(&devbox(), &[]), None);
    assert_eq!(calls.lock().unwrap().len(), 2); // nothing to mirror: no ssh at all
}

#[test]
fn an_rsync_without_append_verify_gets_a_normal_jsonl_pass() {
    let dir = scratch("sync-no-append");
    let (s, calls) = recording(&dir.path().join("state"), false); // e.g. macOS openrsync
    s.sync(&devbox(), &[SID.to_owned()]);
    let seen = calls.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(!seen.iter().flatten().any(|a| a == "--append-verify"));
}

#[test]
fn unsafe_ssh_targets_are_never_used() {
    let dir = scratch("sync-unsafe");
    let (s, calls) = recording(&dir.path().join("state"), true);
    let bad = Machine::new("b", "opt", "-oProxyCommand=x");
    assert_eq!(
        s.sync(&bad, &[SID.to_owned()]).as_deref(),
        Some(UNSAFE_TARGET)
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn sessions_are_canonical_each_once_and_only_session_ids() {
    let dir = scratch("sync-canonical");
    let (s, calls) = recording(&dir.path().join("state"), true);
    let upper = SID.to_uppercase();
    s.sync(&devbox(), &[upper, SID.to_owned(), "../x".to_owned()]);
    let jsonl = calls.lock().unwrap()[0].clone();
    let rules: Vec<_> = jsonl.iter().filter(|a| a.contains(SID)).collect();
    assert_eq!(rules.len(), 4, "{jsonl:?}");
    calls.lock().unwrap().clear();
    s.sync(&devbox(), &["../x".to_owned()]);
    assert!(calls.lock().unwrap().is_empty()); // nothing valid: no ssh
}

#[test]
fn the_mirror_lives_under_the_state_dir_by_machine_id() {
    let m = Machine::new("a/b:c-1", "lab", "lab");
    assert_eq!(
        mirror_root(Path::new("/s"), &m),
        Path::new("/s/remote/abc1/projects")
    );
}

#[test]
fn one_machine_never_syncs_twice_at_once() {
    let dir = scratch("sync-lock");
    let state = dir.path().join("state");
    let (active, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let mut s = RsyncSync::new(&env(&state), Arc::new(|| 3600.0));
    let (a, p) = (active.clone(), peak.clone());
    s.runner = Arc::new(move |_: &[String], _| {
        let now = a.fetch_add(1, Ordering::SeqCst) + 1;
        p.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(100));
        a.fetch_sub(1, Ordering::SeqCst);
        None
    });
    let s = Arc::new(s);
    // a machine id of its own: the lock is per process
    let m = Machine::new("lockprobe1", "lab", "lab");
    let threads: Vec<_> = (0..3)
        .map(|_| {
            let (s, m) = (s.clone(), m.clone());
            std::thread::spawn(move || s.sync(&m, &[SID.to_owned()]))
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1); // one lock per machine around mirror + prune
}
