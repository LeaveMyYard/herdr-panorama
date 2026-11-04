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
            claude_config: Some(self.dir.path().join("claude")),
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
fn the_processes_effective_config_dir_must_be_panoramas() {
    let f = Fixture::new();
    // No CLAUDE_CONFIG_DIR: the process uses $HOME/.claude.
    std::fs::rename(f.dir.path().join("claude"), f.dir.path().join(".claude")).unwrap();
    let mut linker = f.linker();
    linker.claude_config = Some(f.dir.path().join(".claude"));
    let home = format!("HC_TERMINAL_ID=t2\0HOME={}\0", f.dir.path().display());
    f.environ(home.as_bytes());
    let check = |linker: &Linker| linker.per_process("t2", &info("t2", &["claude"]), &f.table);
    assert_eq!(check(&linker), Signal::Names(A.into()));
    // Panorama reads another dir than the process uses, in either direction.
    f.environ(b"HC_TERMINAL_ID=t2\0HOME=/elsewhere\0");
    assert_eq!(check(&linker), Signal::Absent);
    f.environ(format!("{home}CLAUDE_CONFIG_DIR=/elsewhere\0").as_bytes());
    assert_eq!(check(&linker), Signal::Absent);
    // Neither variable: the process's dir is unknown.
    f.environ(b"HC_TERMINAL_ID=t2\0");
    assert_eq!(check(&linker), Signal::Absent);
    // Paths compare by component (a trailing slash is the same dir); links are not resolved.
    f.environ(
        format!(
            "{home}CLAUDE_CONFIG_DIR={}/.claude/\0",
            f.dir.path().display()
        )
        .as_bytes(),
    );
    assert_eq!(check(&linker), Signal::Names(A.into()));
    std::os::unix::fs::symlink(f.dir.path().join(".claude"), f.dir.path().join("alias")).unwrap();
    f.environ(format!("{home}CLAUDE_CONFIG_DIR={}/alias\0", f.dir.path().display()).as_bytes());
    assert_eq!(check(&linker), Signal::Absent);
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
