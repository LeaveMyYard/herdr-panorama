//! Synthetic `/proc` and Claude config directories only; no live process is consulted.
use super::*;
use herdr_agent_kit::agent_procs::{Pid, ProcStat, fake::FakeProcesses};
use serde_json::json;
use std::{cell::Cell, collections::HashMap, path::Path};

const A: &str = "11111111-1111-4111-8111-111111111111";
const B: &str = "22222222-2222-4222-8222-222222222222";

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

struct Fixture {
    dir: tempfile::TempDir,
    table: FakeProcesses,
}

impl Fixture {
    /// Process 20 is `claude`, start token `boot-1`, started in terminal `t2`, and its
    /// per-process record names session A.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("proc/20")).unwrap();
        std::fs::create_dir_all(dir.path().join("claude/sessions")).unwrap();
        std::fs::create_dir_all(dir.path().join("claude/projects")).unwrap();
        let mut fixture = Self {
            dir,
            table: FakeProcesses::default(),
        };
        fixture.table.insert(20, 1, "claude", "boot-1", &["claude"]);
        fixture.environ(&fixture.env_with(b"PATH=/bin\0HC_TERMINAL_ID=t2\0SECRET=never-read\0"));
        fixture.record(
            &json!({"pid":20,"sessionId":A,"procStart":"boot-1","kind":"interactive","cwd":"/x"}),
        );
        fixture
    }
    /// `extra` plus the process's own CLAUDE_CONFIG_DIR, the fixture's config dir.
    fn env_with(&self, extra: &[u8]) -> Vec<u8> {
        let mut bytes = extra.to_vec();
        bytes.extend_from_slice(b"CLAUDE_CONFIG_DIR=");
        bytes.extend_from_slice(self.dir.path().join("claude").to_str().unwrap().as_bytes());
        bytes.push(0);
        bytes
    }
    fn environ(&self, bytes: &[u8]) {
        std::fs::write(self.dir.path().join("proc/20/environ"), bytes).unwrap();
    }
    fn record(&self, value: &Value) {
        std::fs::write(self.sessions().join("20.json"), value.to_string()).unwrap();
    }
    fn sessions(&self) -> std::path::PathBuf {
        self.dir.path().join("claude/sessions")
    }
    fn linker(&self) -> Linker {
        Linker {
            proc: Proc::new(self.dir.path().join("proc"), uid()),
            uid: uid(),
            projects: self.dir.path().join("claude/projects"),
        }
    }
}

fn info(address: &str, argv: &[&str]) -> Value {
    json!({"process_info":{"pane_id":format!("core:{address}"),"foreground_processes":[
        {"pid":20,"name":"claude","argv":argv}]}})
}

fn per_process(f: &Fixture, address: &str) -> Signal {
    f.linker()
        .per_process(address, &info(address, &["claude"]), &f.table)
}

#[test]
fn claudes_own_record_for_the_exact_foreground_process_names_its_session() {
    let f = Fixture::new();
    assert_eq!(per_process(&f, "t2"), Signal::Names(A.into()));
    // A record rewritten after an in-process switch is read again on the next collect.
    f.record(&json!({"pid":20,"sessionId":B,"procStart":"boot-1","kind":"interactive"}));
    assert_eq!(per_process(&f, "t2"), Signal::Names(B.into()));
}

#[test]
fn the_process_gate_refuses_anything_it_cannot_tie_to_the_terminal() {
    let f = Fixture::new();
    // Another terminal, or a remote one.
    assert_eq!(per_process(&f, "t3"), Signal::Absent);
    assert_eq!(per_process(&f, "vps/t2"), Signal::Absent);
    // hc's argv or name disagree with /proc (another namespace, another core instance).
    let linker = f.linker();
    assert_eq!(
        linker.per_process("t2", &info("t2", &["claude", "--x"]), &f.table),
        Signal::Absent
    );
    let mut renamed = info("t2", &["claude"]);
    renamed["process_info"]["foreground_processes"][0]["name"] = json!("node");
    assert_eq!(linker.per_process("t2", &renamed, &f.table), Signal::Absent);
    // Not Claude at all.
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "bash", "boot-1", &["bash"]);
    let shell = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[
        {"pid":20,"name":"bash","argv":["bash"]}]}});
    assert_eq!(linker.per_process("t2", &shell, &table), Signal::Absent);
    // The exec environment must name exactly this terminal, once.
    for environ in [
        &b"HC_TERMINAL_ID=t3\0"[..],
        b"PATH=/bin\0",
        b"HC_TERMINAL_ID=t2\0HC_TERMINAL_ID=t2\0",
    ] {
        f.environ(&f.env_with(environ));
        assert_eq!(per_process(&f, "t2"), Signal::Absent);
    }
    f.environ(&[b'x'; 300 * 1024]);
    assert_eq!(per_process(&f, "t2"), Signal::Absent);
    std::fs::remove_file(f.dir.path().join("proc/20/environ")).unwrap();
    assert_eq!(per_process(&f, "t2"), Signal::Absent);
}

#[test]
fn the_record_is_read_from_the_processes_own_config_dir() {
    let f = Fixture::new();
    let base = f.dir.path().display();
    let check = || per_process(&f, "t2");
    let projects = f.dir.path().join("claude/projects");
    // No CLAUDE_CONFIG_DIR: the process uses $HOME/.claude.
    let default = f.dir.path().join(".claude");
    std::fs::create_dir_all(default.join("sessions")).unwrap();
    std::os::unix::fs::symlink(&projects, default.join("projects")).unwrap();
    std::fs::rename(
        f.sessions().join("20.json"),
        default.join("sessions/20.json"),
    )
    .unwrap();
    let home = format!("HC_TERMINAL_ID=t2\0HOME={base}\0");
    f.environ(home.as_bytes());
    assert_eq!(check(), Signal::Names(A.into()));
    // CLAUDE_CONFIG_DIR wins over $HOME, and its record is the one read.
    let other = f.dir.path().join("claude-3/sessions");
    std::fs::create_dir_all(&other).unwrap();
    std::os::unix::fs::symlink(&projects, f.dir.path().join("claude-3/projects")).unwrap();
    std::fs::write(
        other.join("20.json"),
        json!({"pid":20,"sessionId":B,"procStart":"boot-1","kind":"interactive"}).to_string(),
    )
    .unwrap();
    f.environ(format!("{home}CLAUDE_CONFIG_DIR={base}/claude-3\0").as_bytes());
    assert_eq!(check(), Signal::Names(B.into()));
    // A link to that dir is the same dir: the proof is inside the record, not in the path.
    std::os::unix::fs::symlink(f.dir.path().join("claude-3"), f.dir.path().join("alias")).unwrap();
    f.environ(format!("{home}CLAUDE_CONFIG_DIR={base}/alias/\0").as_bytes());
    assert_eq!(check(), Signal::Names(B.into()));
    // The other gates hold there too: a record of another process, another terminal.
    std::fs::write(
        other.join("20.json"),
        json!({"pid":20,"sessionId":B,"procStart":"boot-0","kind":"interactive"}).to_string(),
    )
    .unwrap();
    f.environ(format!("{home}CLAUDE_CONFIG_DIR={base}/claude-3\0").as_bytes());
    assert_eq!(check(), Signal::Absent);
    std::fs::write(
        other.join("20.json"),
        json!({"pid":20,"sessionId":B,"procStart":"boot-1","kind":"interactive"}).to_string(),
    )
    .unwrap();
    f.environ(format!("HC_TERMINAL_ID=t3\0CLAUDE_CONFIG_DIR={base}/claude-3\0").as_bytes());
    assert_eq!(check(), Signal::Absent);
}

#[test]
fn a_config_dir_with_other_transcripts_than_the_collectors_refuses() {
    let f = Fixture::new();
    let config = f.dir.path().join("claude").display().to_string();
    f.environ(format!("HC_TERMINAL_ID=t2\0CLAUDE_CONFIG_DIR={config}\0").as_bytes());
    let check = |linker: &Linker| linker.per_process("t2", &info("t2", &["claude"]), &f.table);
    assert_eq!(check(&f.linker()), Signal::Names(A.into()));
    // The collector reads another root: an id from here could find a same-named copy there.
    let mut linker = f.linker();
    linker.projects = f.dir.path().join("panoramas-projects");
    assert_eq!(check(&linker), Signal::Absent);
    std::fs::create_dir(f.dir.path().join("panoramas-projects")).unwrap();
    assert_eq!(check(&linker), Signal::Absent);
    // This dir has no transcripts yet.
    std::fs::remove_dir(f.dir.path().join("claude/projects")).unwrap();
    assert_eq!(check(&f.linker()), Signal::Absent);
}

#[test]
fn a_config_dir_that_is_unknown_or_not_absolute_refuses() {
    let f = Fixture::new();
    // The empty value does not fall back to a $HOME that would find the record.
    std::os::unix::fs::symlink(f.dir.path().join("claude"), f.dir.path().join(".claude")).unwrap();
    let home = format!("HOME={}\0", f.dir.path().display());
    f.environ(format!("HC_TERMINAL_ID=t2\0{home}").as_bytes());
    assert_eq!(per_process(&f, "t2"), Signal::Names(A.into()));
    for environ in [
        "HC_TERMINAL_ID=t2\0".to_owned(),
        "HC_TERMINAL_ID=t2\0HOME=relative\0".to_owned(),
        "HC_TERMINAL_ID=t2\0CLAUDE_CONFIG_DIR=relative\0".to_owned(),
        format!("HC_TERMINAL_ID=t2\0CLAUDE_CONFIG_DIR=\0{home}"),
    ] {
        f.environ(environ.as_bytes());
        assert_eq!(per_process(&f, "t2"), Signal::Absent, "{environ:?}");
    }
    let dir = |config: Option<&str>, home: Option<&str>| {
        Environment {
            terminal: None,
            claude_config: config.map(str::to_owned),
            home: home.map(str::to_owned),
        }
        .claude_dir()
    };
    assert_eq!(dir(Some(""), Some("/h")), None);
    assert_eq!(dir(Some("rel"), Some("/h")), None);
    assert_eq!(dir(None, Some("rel")), None);
    assert_eq!(dir(None, None), None);
    assert_eq!(dir(Some("/c"), Some("/h")), Some("/c".into()));
    assert_eq!(dir(None, Some("/h")), Some("/h/.claude".into()));
}

#[test]
fn the_recorded_hc_get_shape_passes_the_gate() {
    let f = Fixture::new();
    let wire: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/hc-get-foreground.json"
    ))
    .unwrap();
    let info = crate::host::core::process(&wire, "t2").unwrap();
    assert_eq!(
        f.linker().per_process("t2", &info, &f.table),
        Signal::Names(A.into())
    );
}

#[test]
fn a_record_from_a_reused_pid_or_an_unsafe_file_is_no_evidence() {
    let f = Fixture::new();
    for record in [
        json!({"pid":20,"sessionId":A,"procStart":"boot-0","kind":"interactive"}),
        json!({"pid":20,"sessionId":A,"procStart":"boot-1","kind":"bg"}),
        json!({"pid":20,"sessionId":A,"procStart":"boot-1"}),
        json!({"pid":20,"sessionId":A,"kind":"interactive"}),
        json!({"pid":20,"sessionId":A,"procStart":1}),
        json!({"pid":21,"sessionId":A,"procStart":"boot-1","kind":"interactive"}),
        json!({"pid":20,"sessionId":"not-a-session","procStart":"boot-1","kind":"interactive"}),
        json!([20]),
    ] {
        f.record(&record);
        assert_eq!(per_process(&f, "t2"), Signal::Absent, "{record}");
    }
    let real = f.sessions().join("elsewhere.json");
    std::fs::write(
        &real,
        json!({"pid":20,"sessionId":A,"procStart":"boot-1","kind":"interactive"}).to_string(),
    )
    .unwrap();
    std::fs::remove_file(f.sessions().join("20.json")).unwrap();
    std::os::unix::fs::symlink(&real, f.sessions().join("20.json")).unwrap();
    assert_eq!(per_process(&f, "t2"), Signal::Absent);
    std::fs::remove_file(f.sessions().join("20.json")).unwrap();
    std::fs::hard_link(&real, f.sessions().join("20.json")).unwrap();
    assert_eq!(per_process(&f, "t2"), Signal::Absent);
    std::fs::remove_file(f.sessions().join("20.json")).unwrap();
    std::fs::write(f.sessions().join("20.json"), vec![b' '; 17 * 1024]).unwrap();
    assert_eq!(per_process(&f, "t2"), Signal::Absent);
}

#[test]
fn a_record_of_another_user_is_no_evidence() {
    let f = Fixture::new();
    let config = f.dir.path().join("claude");
    assert_eq!(
        claude::record(&config, 20, "boot-1", uid()),
        Signal::Names(A.into())
    );
    assert_eq!(
        claude::record(&config, 20, "boot-1", uid() + 1),
        Signal::Absent
    );
}

#[test]
fn a_fifo_in_the_records_place_neither_blocks_nor_counts() {
    let f = Fixture::new();
    let path = f.sessions().join("20.json");
    std::fs::remove_file(&path).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let config = f.dir.path().join("claude");
    let (done, result) = std::sync::mpsc::channel();
    let id = uid();
    std::thread::spawn(move || done.send(claude::record(&config, 20, "boot-1", id)));
    let signal = result.recv_timeout(std::time::Duration::from_secs(10));
    assert_eq!(signal, Ok(Signal::Absent));
}

/// A table whose start token for the PID is gone after the first read (PID reuse mid-read).
struct Reused<'a>(&'a FakeProcesses, Cell<usize>);
impl ProcessTable for Reused<'_> {
    fn stat(&self, pid: Pid) -> Option<ProcStat> {
        self.0.stat(pid)
    }
    fn cmdline(&self, pid: Pid) -> Vec<String> {
        self.0.cmdline(pid)
    }
    fn start(&self, pid: Pid) -> Option<String> {
        self.1.set(self.1.get() + 1);
        (self.1.get() == 1).then(|| self.0.start(pid)).flatten()
    }
}

#[test]
fn a_start_token_that_changes_during_the_read_refuses() {
    let f = Fixture::new();
    let table = Reused(&f.table, Cell::new(0));
    assert_eq!(
        f.linker()
            .per_process("t2", &info("t2", &["claude"]), &table),
        Signal::Absent
    );
    assert_eq!(table.1.get(), 2);
}

fn hook(dir: &Path, session: &str, ts: f64) {
    let line = json!({"v":1,"event":"session","host":"herdr-core","terminal":"t2",
        "harness":"claude","pid":20,"pstart":"boot-1","ts":ts});
    std::fs::write(
        dir.join(format!("{session}.jsonl")),
        line.to_string() + "\n",
    )
    .unwrap();
}

#[test]
fn hook_and_per_process_record_must_agree() {
    let f = Fixture::new();
    let hooks = f.dir.path().join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let env = PluginEnv::from_vars(Vec::<(String, String)>::new(), None, uid());
    let decide = || {
        f.linker()
            .terminal(
                &env,
                "t2",
                "claude",
                &info("t2", &["claude"]),
                &records::load(&hooks),
                &f.table,
            )
            .1
    };
    assert_eq!(decide(), Link::Linked(A.into()));
    hook(&hooks, A, 1.0);
    assert_eq!(decide(), Link::Linked(A.into()));
    // In-process switch: Claude rewrote its record, the hook record has not landed.
    f.record(&json!({"pid":20,"sessionId":B,"procStart":"boot-1","kind":"interactive"}));
    assert_eq!(decide(), Link::Conflict);
    // It lands: the newest hook session agrees with the record.
    hook(&hooks, B, 2.0);
    assert_eq!(decide(), Link::Linked(B.into()));
    // Without the per-process record, two hook sessions stay ambiguous.
    std::fs::remove_file(f.sessions().join("20.json")).unwrap();
    assert_eq!(decide(), Link::Unavailable);
}

/// A long-lived session's file: its resume record with subagent lines around it, past the
/// size a hook file reaches in practice.
fn big_hook(dir: &Path, session: &str, ts: f64) {
    let subagent = json!({"v":1,"event":"start","agent":"a","host":"herdr-core","terminal":"t2",
        "harness":"claude","pid":20,"pstart":"boot-1","ts":ts})
    .to_string();
    let resume = json!({"v":1,"event":"session","host":"herdr-core","terminal":"t2",
        "harness":"claude","pid":20,"pstart":"boot-1","ts":ts,"source":"resume"})
    .to_string();
    let mut text = format!("{subagent}\n").repeat(1000);
    text += &format!("{resume}\n");
    text += &format!("{subagent}\n").repeat(4000);
    assert!(text.len() > 512 * 1024);
    std::fs::write(dir.join(format!("{session}.jsonl")), text).unwrap();
}

struct Restart<'a> {
    f: &'a Fixture,
    hooks: std::path::PathBuf,
}

impl<'a> Restart<'a> {
    /// `claude`, then an in-process resume: `startup` for session A in a new small file, and
    /// `resume` for B, the session Claude's own record names.
    fn new(f: &'a Fixture) -> Self {
        let hooks = f.dir.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        hook(&hooks, A, 1.0);
        f.record(&json!({"pid":20,"sessionId":B,"procStart":"boot-1","kind":"interactive"}));
        Self { f, hooks }
    }
    fn decide(&self) -> Link {
        let env = PluginEnv::from_vars(Vec::<(String, String)>::new(), None, uid());
        let hooks = records::load(&self.hooks);
        let info = info("t2", &["claude"]);
        (self.f.linker())
            .terminal(&env, "t2", "claude", &info, &hooks, &self.f.table)
            .1
    }
    fn lose_per_process_record(&self) {
        std::fs::remove_file(self.f.sessions().join("20.json")).unwrap();
    }
    /// A hook file over the per-file limit; sparse, so the test writes nothing.
    fn oversized(&self, session: &str) {
        let file = std::fs::File::create(self.hooks.join(format!("{session}.jsonl"))).unwrap();
        file.set_len(17 * 1024 * 1024).unwrap();
    }
}

#[test]
fn a_big_hook_file_keeps_its_resume_record_after_a_restart() {
    let f = Fixture::new();
    let restart = Restart::new(&f);
    big_hook(&restart.hooks, B, 2.0);
    assert_eq!(restart.decide(), Link::Linked(B.into()));
    // Alone, the two hook sessions stay ambiguous: neither is named, the throwaway least of all.
    restart.lose_per_process_record();
    assert_eq!(restart.decide(), Link::Unavailable);
}

#[test]
fn an_unread_hook_file_never_lets_the_throwaway_id_link() {
    let f = Fixture::new();
    let restart = Restart::new(&f);
    restart.oversized(B);
    // The resume record is out of reach and Claude names B: refused, A is not linked.
    assert_eq!(restart.decide(), Link::Conflict);
    // Without Claude's record only the throwaway id is left, and it does not name itself.
    restart.lose_per_process_record();
    assert_eq!(restart.decide(), Link::Unavailable);
}

#[test]
fn an_unread_hook_file_leaves_agreeing_evidence_and_the_record_alone_linking() {
    let f = Fixture::new();
    let restart = Restart::new(&f);
    restart.oversized("33333333-3333-4333-8333-333333333333");
    // What was read agrees with Claude: as with a lost hook write, the two together link.
    f.record(&json!({"pid":20,"sessionId":A,"procStart":"boot-1","kind":"interactive"}));
    assert_eq!(restart.decide(), Link::Linked(A.into()));
    // The process's only hook record is in the unread file: Claude's record stands alone.
    std::fs::remove_file(restart.hooks.join(format!("{A}.jsonl"))).unwrap();
    assert_eq!(restart.decide(), Link::Linked(A.into()));
}

#[test]
fn one_session_or_one_process_in_two_terminals_refuses_both() {
    let mut links = HashMap::from([
        ("core:t1".to_owned(), (Some(20), Link::Linked(A.into()))),
        ("core:t2".to_owned(), (Some(21), Link::Linked(A.into()))),
        ("core:t3".to_owned(), (Some(22), Link::Linked(B.into()))),
    ]);
    unique(&mut links);
    assert_eq!(links["core:t1"].1, Link::Unavailable);
    assert_eq!(links["core:t2"].1, Link::Unavailable);
    assert_eq!(links["core:t3"].1, Link::Linked(B.into()));
    let mut links = HashMap::from([
        ("core:t1".to_owned(), (Some(20), Link::Linked(A.into()))),
        ("core:t2".to_owned(), (Some(20), Link::Unavailable)),
        ("vps/t3".to_owned(), (None, Link::Linked(B.into()))),
    ]);
    unique(&mut links);
    assert_eq!(links["core:t1"].1, Link::Unavailable);
    assert_eq!(links["vps/t3"].1, Link::Linked(B.into()));
}
