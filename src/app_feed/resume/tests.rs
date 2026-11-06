//! A scripted hc world: every "must not type" case asserts on the sends it recorded.
use super::{
    decide::{self, CONFIRM_MS, ENDING_MS, FRESH_CORE_MS, SETTLE_MS, SHELL_WAIT_MS},
    hc::{Foreground as Fg, Hc, Lookup, Sent, Status, Term},
    state::{Launch, Record, Saved, Stage},
    toggle::TOGGLE_STORE,
    *,
};
use crate::app_feed::fs::Home;
use serde_json::json;
use std::{cell::Cell, cell::RefCell, collections::BTreeMap, os::unix::fs::PermissionsExt, rc::Rc};

const ID: &str = "0123456789abcdef0123456789abcdef";
const GEN: &str = "data/panorama/runtime/0123456789abcdef0123456789abcdef";
const SESSION: &str = "11111111-2222-4333-8444-555555555555";
const OTHER: &str = "99999999-2222-4333-8444-555555555555";
const CWD: &str = "/fixture/project-alpha";

thread_local! {
    static NOW: Cell<u64> = const { Cell::new(1_000_000) };
    static BOOT: Cell<u32> = const { Cell::new(1) };
}
fn clock() -> u64 {
    NOW.with(Cell::get)
}
fn boot() -> Option<String> {
    Some(format!("boot-{}", BOOT.with(Cell::get)))
}
const CLOCKS: Sources = Sources {
    clock,
    mono: clock,
    boot,
};
fn fg(pid: u32, cwd: &str) -> Fg {
    let shell = pid == 20 || pid == 77;
    Fg {
        pid,
        name: if shell { "bash" } else { "claude" }.into(),
        argv: if shell {
            vec!["/bin/bash".into()]
        } else {
            vec!["claude".into()]
        },
        cwd: cwd.into(),
    }
}
fn pass(ms: u64) {
    NOW.with(|n| n.set(n.get() + ms));
}

struct World {
    status: Option<Status>,
    terms: BTreeMap<String, Lookup>,
    screens: BTreeMap<String, String>,
    send: Option<Sent>,
    sends: Vec<(String, String)>,
    calls: usize,
    quiet: bool,
}
impl Default for World {
    fn default() -> Self {
        Self {
            status: None,
            terms: BTreeMap::new(),
            screens: BTreeMap::new(),
            send: None,
            sends: Vec::new(),
            calls: 0,
            quiet: true,
        }
    }
}
#[derive(Clone, Default)]
struct Fake(Rc<RefCell<World>>);
impl Hc for Fake {
    fn status(&self) -> Option<Status> {
        self.0.borrow_mut().calls += 1;
        self.0.borrow().status.clone()
    }
    fn get(&self, address: &str) -> Lookup {
        self.0.borrow_mut().calls += 1;
        self.0
            .borrow()
            .terms
            .get(address)
            .cloned()
            .unwrap_or(Lookup::Gone)
    }
    fn screen(&self, address: &str) -> Option<String> {
        self.0.borrow_mut().calls += 1;
        self.0.borrow().screens.get(address).cloned()
    }
    fn quiet(&self, _: &str) -> bool {
        self.0.borrow().quiet
    }
    fn send(&self, address: &str, text: &str) -> Sent {
        let mut world = self.0.borrow_mut();
        world.sends.push((address.into(), text.into()));
        world.send.clone().unwrap_or(Sent::Accepted)
    }
}

/// The agent as hc shows it before the loss: a shell (20) with Claude (30) in front.
fn agent_term() -> Term {
    Term {
        live: true,
        keep: true,
        pid: Some(20),
        reason: None,
        created_ms: 5,
        argv: vec!["/bin/bash".into()],
        reports: true,
        foreground: Some(fg(30, CWD)),
    }
}
/// The recovery shell: a new program, in front of nothing.
fn shell_term() -> Term {
    Term {
        pid: Some(77),
        reports: false,
        foreground: Some(fg(77, CWD)),
        ..agent_term()
    }
}
/// Lost, with its new shell not started yet.
fn waiting() -> Term {
    Term {
        pid: None,
        reason: Some("stopped_by_restart".into()),
        reports: false,
        foreground: None,
        ..agent_term()
    }
}
fn link(address: &str, session: &str, argv: &[&str]) -> Linked {
    Linked {
        address: address.into(),
        harness: "claude".into(),
        session: session.into(),
        default_config: true,
        terminal: json!({"state":"live","keep":true,"pid":20,"created_ms":5,"argv":["/bin/bash"],
            "status":{"state":"idle"},"reason":null,
            "foreground":{"pid":30,"cwd":CWD,"name":"claude","argv":argv}}),
    }
}
fn status(holder: &str) -> Status {
    Status {
        core: "c0ffee".into(),
        uptime_ms: 1000,
        holders: vec![holder.into()],
    }
}

struct Rig {
    _dir: tempfile::TempDir,
    home: Home,
    fake: Fake,
    resume: Resume<Fake>,
    locks: PathBuf,
}
fn on(home: &Home) {
    let path = home.path.join(TOGGLE_STORE);
    std::fs::write(&path, r#"{"agents.resume":"\"on\""}"#).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join(GEN)).unwrap();
        std::fs::create_dir_all(root.join("data/panorama-spaces")).unwrap();
        let home = Home::open(&root).unwrap();
        on(&home);
        let fake = Fake::default();
        fake.0.borrow_mut().status = Some(status("holder-a"));
        let locks = root.join("locks");
        let resume = Resume::with(fake.clone(), ID, GEN, locks.clone(), CLOCKS);
        Self {
            _dir: dir,
            home,
            fake,
            resume,
            locks,
        }
    }
    fn tick(&mut self, linked: &[Linked]) -> Option<Value> {
        self.resume.tick(&self.home, linked, &|| true)
    }
    fn world(&self) -> std::cell::RefMut<'_, World> {
        self.fake.0.borrow_mut()
    }
    fn sends(&self) -> Vec<(String, String)> {
        self.fake.0.borrow().sends.clone()
    }
    fn set(&self, term: Term) {
        self.world().terms.insert("t5".into(), Lookup::Found(term));
    }
    fn saved(&self) -> Saved {
        let bytes = std::fs::read(self.home.path.join(GEN).join(store::FILE)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
    /// A reboot: new holders, a new boot id, and the terminal still waiting for its shell.
    fn reboot(&self, holder: &str) {
        BOOT.with(|b| b.set(b.get() + 1));
        self.world().status = Some(status(holder));
        self.set(waiting());
    }
    /// A watched agent, then a reboot seen while the shell is on its way, then this terminal.
    fn lost(&mut self, term: Term) -> Option<Value> {
        self.set(agent_term());
        self.tick(&[link("t5", SESSION, &["claude"])]);
        self.reboot("holder-b");
        self.tick(&[]);
        self.world().screens.insert("t5".into(), "$ ".into());
        self.set(term);
        self.tick(&[])
    }
    /// Enough ticks for a settled shell to be typed into.
    fn settle(&mut self) -> Option<Value> {
        pass(SETTLE_MS);
        self.tick(&[])
    }
    fn stage(&self) -> Stage {
        self.saved().loss.unwrap().entries["t5"].stage
    }
    fn reason(&self) -> String {
        self.saved().loss.unwrap().entries["t5"]
            .reason
            .clone()
            .unwrap_or_default()
    }
}

#[test]
fn a_real_loss_types_the_resume_command_once_and_confirms_it() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    assert!(
        rig.sends().is_empty(),
        "first sight of a shell is never enough"
    );
    rig.settle();
    assert_eq!(
        rig.sends(),
        [("t5".into(), format!("claude --resume {SESSION}"))]
    );
    rig.set(Term {
        foreground: Some(fg(90, CWD)),
        ..shell_term()
    });
    let report = rig
        .tick(&[link("t5", SESSION, &["claude", "--resume", SESSION])])
        .unwrap();
    assert_eq!(rig.stage(), Stage::Resumed);
    assert_eq!(report["loss"]["entries"][0]["state"], "resumed");
    assert_eq!(report["loss"]["entries"][0]["terminal"], "t5");
    pass(CONFIRM_MS * 2);
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn off_runs_nothing_types_nothing_and_leaves_no_record() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    assert!(rig.home.path.join(GEN).join(store::FILE).exists());
    std::fs::remove_file(rig.home.path.join(TOGGLE_STORE)).unwrap();
    rig.world().calls = 0;
    rig.reboot("holder-b");
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    assert_eq!(rig.tick(&[link("t5", SESSION, &["claude"])]), None);
    rig.settle();
    assert_eq!(rig.world().calls, 0, "no hc call at all");
    assert!(rig.sends().is_empty());
    assert!(!rig.home.path.join(GEN).join(store::FILE).exists());
    // Turned on after the loss: nothing old is left to act on.
    on(&rig.home);
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
}

#[test]
fn an_unreadable_toggle_neither_types_nor_clears_the_record() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    std::fs::write(rig.home.path.join(TOGGLE_STORE), "{broken").unwrap();
    assert_eq!(rig.tick(&[]), None);
    assert_eq!(rig.saved().records.len(), 1);
}

#[test]
fn the_toggle_is_read_again_right_before_typing() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    let home = rig.home.path.clone();
    pass(SETTLE_MS);
    // Off between this tick's first read of the toggle and the send.
    struct Flip(Fake, PathBuf);
    impl Hc for Flip {
        fn status(&self) -> Option<Status> {
            self.0.status()
        }
        fn get(&self, a: &str) -> Lookup {
            self.0.get(a)
        }
        fn screen(&self, a: &str) -> Option<String> {
            let _ = std::fs::remove_file(self.1.join(TOGGLE_STORE));
            self.0.screen(a)
        }
        fn quiet(&self, a: &str) -> bool {
            self.0.quiet(a)
        }
        fn send(&self, a: &str, t: &str) -> Sent {
            self.0.send(a, t)
        }
    }
    let mut flip = Resume::with(
        Flip(rig.fake.clone(), home),
        ID,
        GEN,
        rig.locks.clone(),
        CLOCKS,
    );
    rig.resume.held = None;
    flip.saved = rig.resume.saved.clone();
    flip.started = true;
    flip.store.load(&rig.home, ID).unwrap();
    flip.settled = rig.resume.settled.clone();
    flip.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
}

#[test]
fn a_stopping_or_uninstalled_writer_does_not_type() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS);
    rig.resume.tick(&rig.home, &[], &|| false);
    assert!(rig.sends().is_empty());
}

#[test]
fn the_same_holders_mean_no_loss_whatever_else_changed() {
    let saved = Saved {
        core: Some("c0ffee".into()),
        holders: vec!["a".into(), "b".into()],
        ..Saved::new(ID)
    };
    let with = |holders: &[&str]| Status {
        holders: holders.iter().map(|h| (*h).to_owned()).collect(),
        ..status("x")
    };
    // App restart, `hc server restart`, a core crash: the holders survive.
    assert_eq!(decide::compare(&saved, &with(&["a", "b"])), Change::Same);
    // An old holder draining after an update.
    assert_eq!(decide::compare(&saved, &with(&["b"])), Change::Same);
    assert_eq!(decide::compare(&saved, &with(&[])), Change::NoHolderYet);
    assert_eq!(decide::compare(&saved, &with(&["c"])), Change::Lost);
    let other = Status {
        core: "beef".into(),
        ..with(&["c"])
    };
    assert_eq!(decide::compare(&saved, &other), Change::Foreign);
    assert_eq!(
        decide::compare(&Saved::new(ID), &with(&["c"])),
        Change::Same
    );
}

#[test]
fn an_app_restart_with_terminals_still_running_types_nothing() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.resume.close(&rig.home);
    // The app reopens: same holder. The person quit the agent meanwhile, at the same shell.
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    let shell = Term {
        pid: Some(20),
        reports: false,
        foreground: Some(fg(20, CWD)),
        ..agent_term()
    };
    rig.set(shell);
    rig.world().screens.insert("t5".into(), "$ ".into());
    for _ in 0..4 {
        next.tick(&rig.home, &[], &|| true);
        pass(ENDING_MS);
    }
    assert!(rig.sends().is_empty());
    assert!(
        rig.saved().records.is_empty(),
        "the quit session is dropped"
    );
}

#[test]
fn a_kept_program_dying_alone_is_not_a_core_loss() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // Same holder, new shell: hc's `signaled` recovery.
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    for _ in 0..4 {
        rig.tick(&[]);
        pass(ENDING_MS);
    }
    assert!(rig.sends().is_empty());
    assert!(rig.saved().loss.is_none());
    assert!(rig.saved().records.is_empty());
}

#[test]
fn a_quit_session_does_not_come_back_after_a_later_loss() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.set(Term {
        reports: false,
        foreground: Some(fg(20, CWD)),
        ..agent_term()
    });
    rig.tick(&[]);
    pass(ENDING_MS);
    rig.tick(&[]);
    rig.reboot("holder-b");
    rig.tick(&[]);
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert!(rig.saved().loss.is_none());
}

#[test]
fn a_session_gone_moments_before_the_loss_is_shown_not_typed() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.set(Term {
        reports: false,
        foreground: Some(fg(20, CWD)),
        ..agent_term()
    });
    rig.tick(&[]);
    rig.reboot("holder-b");
    rig.tick(&[]);
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("already gone"));
}

#[test]
fn a_missing_agent_is_kept_through_the_cores_own_stop_window() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.set(Term {
        reports: false,
        foreground: Some(fg(20, CWD)),
        ..agent_term()
    });
    rig.tick(&[]);
    pass(ENDING_MS - 1);
    rig.tick(&[]);
    assert_eq!(rig.saved().records.len(), 1, "still inside the window");
    pass(1);
    rig.tick(&[]);
    assert!(rig.saved().records.is_empty());
}

#[test]
fn a_terminal_that_ended_normally_ends_its_record() {
    for reason in ["exited", "killed"] {
        let mut rig = Rig::new();
        rig.set(agent_term());
        rig.tick(&[link("t5", SESSION, &["claude"])]);
        rig.set(Term {
            live: false,
            reason: Some(reason.into()),
            foreground: None,
            ..agent_term()
        });
        rig.tick(&[]);
        assert!(rig.saved().records.is_empty(), "{reason}");
    }
}

#[test]
fn reads_that_say_nothing_never_end_a_record() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().terms.insert("t5".into(), Lookup::Unknown);
    pass(ENDING_MS * 3);
    rig.tick(&[]);
    assert_eq!(rig.saved().records["t5"].ending_ms, None);
    // Mid-stop: the core says the program is lost, with no foreground to compare.
    for reason in [
        "server_stop",
        "stopped_by_restart",
        "holder_lost",
        "signaled",
    ] {
        rig.set(Term {
            pid: None,
            reason: Some(reason.into()),
            foreground: None,
            ..agent_term()
        });
        rig.tick(&[]);
        pass(ENDING_MS * 3);
        rig.tick(&[]);
        assert_eq!(rig.saved().records.len(), 1, "{reason}");
        assert_eq!(rig.saved().records["t5"].ending_ms, None, "{reason}");
    }
    rig.status_down();
    pass(ENDING_MS * 3);
    let report = rig.tick(&[]).unwrap();
    assert_eq!(report["tracked"], 1);
    assert!(report["error"].as_str().unwrap().contains("unavailable"));
}

impl Rig {
    fn status_down(&self) {
        self.world().status = None;
    }
}

#[test]
fn a_closed_writer_cannot_prove_the_session_was_live() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.resume.close(&rig.home);
    assert!(rig.saved().closed);
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    rig.reboot("holder-b");
    next.tick(&rig.home, &[], &|| true);
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    next.tick(&rig.home, &[], &|| true);
    pass(SETTLE_MS);
    let report = next.tick(&rig.home, &[], &|| true).unwrap();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    // It can still be resumed: the report says how, and offers the click.
    let entry = &report["loss"]["entries"][0];
    assert_eq!(entry["state"], "skipped");
    assert_eq!(entry["click"], true);
    assert_eq!(entry["command"], format!("claude --resume {SESSION}"));
}

#[test]
fn a_killed_writer_still_proves_a_reboot() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // No close: the machine went down with it. The app opens first, then the shell returns.
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    rig.reboot("unused");
    rig.world().status = Some(Status {
        holders: vec![],
        ..status("x")
    });
    next.tick(&rig.home, &[], &|| true);
    assert!(next.busy(), "looks again soon");
    rig.world().status = Some(status("holder-b"));
    next.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty(), "no shell yet");
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    next.tick(&rig.home, &[], &|| true);
    pass(SETTLE_MS);
    next.tick(&rig.home, &[], &|| true);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_restarted_writer_must_prove_each_session_again() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    next.tick(&rig.home, &[], &|| true);
    assert!(!rig.saved().records["t5"].proved);
    next.tick(&rig.home, &[link("t5", SESSION, &["claude"])], &|| true);
    assert!(rig.saved().records["t5"].proved);
}

#[test]
fn a_restarted_writer_that_cannot_read_still_stops_trusting_the_record() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    rig.world().terms.insert("t5".into(), Lookup::Unknown);
    next.tick(&rig.home, &[], &|| true);
    assert!(!rig.saved().records["t5"].proved);
}

fn exec(name: &str, argv: &[&str]) -> Term {
    let mut foreground = fg(77, CWD);
    foreground.name = name.into();
    foreground.argv = argv.iter().map(|a| (*a).to_owned()).collect();
    Term {
        foreground: Some(foreground),
        ..shell_term()
    }
}

#[test]
fn every_busy_or_doubtful_terminal_is_shown_and_never_typed_into() {
    let running = Some(fg(88, CWD));
    let cases: Vec<(&str, Term, &str)> = vec![
        (
            "foreground program",
            Term {
                foreground: running,
                ..shell_term()
            },
            "before its shell was ready",
        ),
        (
            "reports status",
            Term {
                reports: true,
                ..shell_term()
            },
            "before its shell was ready",
        ),
        (
            "wrong folder",
            Term {
                foreground: Some(fg(77, "/fixture/other")),
                ..shell_term()
            },
            "folder",
        ),
        (
            "different terminal",
            Term {
                created_ms: 6,
                ..shell_term()
            },
            "different terminal",
        ),
        // `exec` keeps the pid (B2): only the words differ.
        (
            "exec python3",
            exec("python3", &["python3"]),
            "isn't a shell",
        ),
        ("exec ssh", exec("ssh", &["ssh", "box"]), "isn't a shell"),
        (
            "exec bash -c",
            exec("bash", &["bash", "-c", "read x"]),
            "isn't a shell",
        ),
        (
            "unknown shell",
            Term {
                argv: vec!["nu".into()],
                ..exec("nu", &["nu"])
            },
            "isn't a shell",
        ),
        (
            "finished",
            Term {
                live: false,
                ..shell_term()
            },
            "finished",
        ),
        (
            "not kept",
            Term {
                keep: false,
                ..shell_term()
            },
            "kept",
        ),
    ];
    for (name, term, reason) in cases {
        let mut rig = Rig::new();
        rig.lost(term);
        rig.settle();
        // Back to a bare shell later: the skip is final, the person has used it.
        rig.set(shell_term());
        rig.settle();
        rig.settle();
        assert!(rig.sends().is_empty(), "{name}");
        assert_eq!(rig.stage(), Stage::Skipped, "{name}");
        assert!(rig.reason().contains(reason), "{name}: {}", rig.reason());
    }
}

#[test]
fn a_missing_terminal_is_skipped() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.reboot("holder-b");
    rig.world().terms.clear();
    rig.tick(&[]);
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn a_shell_that_has_not_drawn_is_not_typed_into() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.reboot("holder-b");
    rig.tick(&[]);
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "   \n".into());
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Waiting);
    // Past the give-up nothing is typed, however quiet it becomes.
    pass(SHELL_WAIT_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
}

#[test]
fn two_equal_reads_closer_than_the_settle_time_do_not_type() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS - 1);
    rig.tick(&[]);
    assert!(rig.sends().is_empty(), "one ms short");
    pass(1);
    rig.tick(&[]);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_half_typed_line_is_never_submitted() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    // Typed, then still: "unchanged" must not be enough.
    let half = "$ ./deploy.sh prod ";
    rig.world().screens.insert("t5".into(), half.into());
    rig.settle();
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("screen changed"));
}

#[test]
fn a_terminal_used_since_the_loss_is_a_final_skip() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    let used = "$ echo I_AM_WORKING_HERE\nI_AM_WORKING_HERE\n$ ";
    rig.world().screens.insert("t5".into(), used.into());
    rig.settle();
    // Cleared again: it looks untouched now, but the skip stands.
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn a_change_undone_inside_the_settle_time_still_counts() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(100);
    rig.world().screens.insert("t5".into(), "$ x".into());
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.settle();
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn a_shell_first_seen_already_drawn_cannot_prove_its_line_is_empty() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // Panorama arrives after the shell is up: a half line could be in this first screen.
    BOOT.with(|b| b.set(b.get() + 1));
    rig.world().status = Some(status("holder-b"));
    rig.set(shell_term());
    rig.world()
        .screens
        .insert("t5".into(), "$ rm -rf build ".into());
    rig.tick(&[]);
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("its line is unproved"));
}

#[test]
fn someone_typing_stops_the_send_and_is_final() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.world().quiet = false;
    rig.settle();
    rig.world().quiet = true;
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("typing"));
}

#[test]
fn the_last_reads_come_after_the_attempt_is_saved() {
    struct Order(Fake, PathBuf);
    impl Hc for Order {
        fn status(&self) -> Option<Status> {
            self.0.status()
        }
        fn get(&self, a: &str) -> Lookup {
            self.0.get(a)
        }
        fn screen(&self, a: &str) -> Option<String> {
            self.0.screen(a)
        }
        fn quiet(&self, a: &str) -> bool {
            let saved: Saved = serde_json::from_slice(&std::fs::read(&self.1).unwrap()).unwrap();
            assert_eq!(saved.loss.unwrap().entries["t5"].stage, Stage::Sending);
            self.0.0.borrow_mut().calls = 0;
            self.0.quiet(a)
        }
        fn send(&self, a: &str, t: &str) -> Sent {
            let reads = self.0.0.borrow().calls;
            assert_eq!(reads, 2, "a get and a screen after the save");
            self.0.send(a, t)
        }
    }
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS);
    let path = rig.home.path.join(GEN).join(store::FILE);
    let order = Order(rig.fake.clone(), path);
    let mut next = Resume::with(order, ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    next.saved = rig.resume.saved.clone();
    next.started = true;
    next.store.load(&rig.home, ID).unwrap();
    next.settled = rig.resume.settled.clone();
    next.tick(&rig.home, &[], &|| true);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_program_appearing_at_the_last_look_stops_the_send() {
    struct Late(Fake);
    impl Hc for Late {
        fn status(&self) -> Option<Status> {
            self.0.status()
        }
        fn get(&self, a: &str) -> Lookup {
            self.0.get(a)
        }
        fn screen(&self, a: &str) -> Option<String> {
            // The screen read passes; the very next read shows a program.
            let found = Lookup::Found(Term {
                foreground: Some(fg(88, CWD)),
                ..shell_term()
            });
            self.0.0.borrow_mut().terms.insert("t5".into(), found);
            self.0.screen(a)
        }
        fn quiet(&self, a: &str) -> bool {
            self.0.quiet(a)
        }
        fn send(&self, a: &str, t: &str) -> Sent {
            self.0.send(a, t)
        }
    }
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS);
    let mut late = Resume::with(Late(rig.fake.clone()), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    late.saved = rig.resume.saved.clone();
    late.started = true;
    late.store.load(&rig.home, ID).unwrap();
    late.settled = rig.resume.settled.clone();
    late.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
}

#[test]
fn only_plain_interactive_launches_are_typed() {
    let run = |argv: &[&str]| {
        let mut rig = Rig::new();
        rig.set(agent_term());
        rig.tick(&[link("t5", SESSION, argv)]);
        rig.reboot("holder-b");
        rig.tick(&[]);
        rig.world().screens.insert("t5".into(), "$ ".into());
        rig.set(shell_term());
        rig.tick(&[]);
        rig.settle();
        rig
    };
    let shown: &[&[&str]] = &[
        &["claude", "rc"],
        &["claude", "--verbose", "remote-control"],
        &["claude", "--remote-control"],
        &["claude", "--remote-control", "phone"],
        &["claude", "--remote-control=phone"],
        &["claude", "-p", "do it"],
        &["claude", "--print"],
        &["claude", "--bg"],
        &["claude", "--tmux", "-w"],
        &["claude", "--permission-mode", "plan"],
        &["claude", "a task"],
        &["claude", "--no-such-flag"],
        &["claude", "--resume", "not-a-uuid"],
    ];
    for argv in shown {
        let rig = run(argv);
        assert!(rig.sends().is_empty(), "{argv:?}");
        assert!(rig.reason().contains("can't repeat"), "{argv:?}");
    }
    let typed: &[&[&str]] = &[
        &["claude"],
        &["claude", "--resume", SESSION],
        &["claude", "-r", SESSION],
        &["claude", "--continue"],
        &["claude", "--model", "opus"],
    ];
    for argv in typed {
        assert_eq!(run(argv).sends().len(), 1, "{argv:?}");
    }
}

#[test]
fn dropped_options_are_noted_on_the_row() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude", "--model", "opus"])]);
    rig.reboot("holder-b");
    let report = rig.tick(&[]).unwrap();
    let note = report["loss"]["entries"][0]["note"].as_str().unwrap();
    assert!(note.contains("aren't repeated"));
    let mut rig = Rig::new();
    rig.lost(shell_term());
    let report = rig.tick(&[]).unwrap();
    assert_eq!(report["loss"]["entries"][0]["note"], Value::Null);
}

fn losing_without_reboot(rig: &mut Rig) {
    losing_at(rig, "$ ");
}

fn losing_at(rig: &mut Rig, screen: &str) {
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().status = Some(status("holder-b"));
    rig.set(waiting());
    rig.tick(&[]);
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), screen.into());
    rig.tick(&[]);
    rig.settle();
}

fn click(rig: &Rig, at: u64) {
    let loss = rig.saved().loss.unwrap().detected_ms;
    press(rig, loss, None, at);
}

fn press(rig: &Rig, loss: u64, token: Option<String>, at: u64) {
    let clicks = json!({"t5": {"loss": loss, "token": token, "at": at}}).to_string();
    let store = json!({"agents.resume": "\"on\"", "agents.resume.clicks": clicks});
    std::fs::write(rig.home.path.join(TOGGLE_STORE), store.to_string()).unwrap();
}

#[test]
fn a_stop_without_a_reboot_is_never_typed_by_itself() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    rig.settle();
    pass(CONFIRM_MS);
    rig.tick(&[]);
    assert!(rig.sends().is_empty(), "nobody asked");
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("without a reboot"));
}

#[test]
fn no_boot_id_means_a_click_too() {
    fn none() -> Option<String> {
        None
    }
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // A boot id on record and none now, as when /proc can't be read.
    rig.resume.boot = none;
    rig.world().status = Some(status("holder-b"));
    rig.set(waiting());
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

/// The two presses: looks, then returns the view the row would show.
fn looked(rig: &mut Rig) -> Value {
    pass(1);
    click(rig, clock());
    let report = rig.tick(&[]).unwrap();
    report["loss"]["entries"][0]["look"].clone()
}

fn confirm(rig: &mut Rig, view: &Value) {
    pass(1);
    let loss = rig.saved().loss.unwrap().detected_ms;
    press(
        rig,
        loss,
        view["token"].as_str().map(str::to_owned),
        clock(),
    );
    rig.tick(&[]);
}

#[test]
fn the_first_click_only_looks_and_the_second_types_once() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    let view = looked(&mut rig);
    assert!(rig.sends().is_empty(), "a first click types nothing");
    assert_eq!(view["lines"], json!(["$"]));
    confirm(&mut rig, &view);
    let typed = format!("claude --resume {SESSION}");
    assert_eq!(rig.sends(), [("t5".into(), typed)]);
    rig.tick(&[]);
    pass(CONFIRM_MS);
    rig.tick(&[]);
    pass(CONFIRM_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Failed);
    assert_eq!(rig.sends().len(), 1, "the same click acts once");
}

#[test]
fn a_half_line_is_shown_before_it_can_be_confirmed() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    rig.world()
        .screens
        .insert("t5".into(), "x\n$ rm -rf build".into());
    let view = looked(&mut rig);
    assert_eq!(
        view["lines"],
        json!(["x", "$ rm -rf build"]),
        "the person sees their line"
    );
    // Typing on after the look: the screen no longer matches what was shown.
    rig.world()
        .screens
        .insert("t5".into(), "x\n$ rm -rf build/".into());
    confirm(&mut rig, &view);
    assert!(rig.sends().is_empty());
    assert!(rig.reason().contains("screen changed"));
    // A view of the line as it is now can be confirmed.
    let view = looked(&mut rig);
    assert_eq!(view["lines"], json!(["x", "$ rm -rf build/"]));
    confirm(&mut rig, &view);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_ticking_clock_can_be_confirmed_but_a_typed_digit_cannot() {
    let clock_at = |tick: &str| format!("12:00:0{tick} $");
    let mut rig = Rig::new();
    losing_at(&mut rig, &clock_at("1"));
    let view = looked(&mut rig);
    rig.world().screens.insert("t5".into(), clock_at("2"));
    confirm(&mut rig, &view);
    assert_eq!(rig.sends().len(), 1);
    let mut rig = Rig::new();
    losing_at(&mut rig, &clock_at("1"));
    let view = looked(&mut rig);
    rig.world()
        .screens
        .insert("t5".into(), format!("{} 7", clock_at("1")));
    confirm(&mut rig, &view);
    assert!(rig.sends().is_empty());
    // A digit typed over a space before a right prompt, which keeps the length.
    let mut rig = Rig::new();
    losing_at(&mut rig, "$     12:00:01");
    let view = looked(&mut rig);
    rig.world()
        .screens
        .insert("t5".into(), "$ 7   12:00:01".into());
    confirm(&mut rig, &view);
    assert!(rig.sends().is_empty());
}

#[test]
fn a_view_is_only_offered_when_the_last_line_is_the_prompt() {
    let cases = [
        ("a listing below the prompt", "$ touch file\nfileA  fileB"),
        ("a continuation", "$ touch x \\\n>"),
        ("a zsh continuation", "$ echo \"\ndquote>"),
        ("a fish continuation", "$ touch x \\\n\n"),
    ];
    for (name, screen) in cases {
        let mut rig = Rig::new();
        losing_without_reboot(&mut rig);
        rig.world().screens.insert("t5".into(), screen.into());
        assert_eq!(looked(&mut rig), Value::Null, "{name}");
        assert!(rig.reason().contains("isn't the prompt"), "{name}");
        // Clearing it makes the button work again.
        rig.world().screens.insert("t5".into(), "$ ".into());
        assert!(looked(&mut rig).is_object(), "{name}");
    }
}

#[test]
fn a_view_keeps_the_end_of_long_lines_and_the_last_few() {
    let mut rig = Rig::new();
    let long = format!("{}$", "p".repeat(400));
    losing_at(&mut rig, &format!("a\nb\nc\nd\n{long}"));
    rig.world()
        .screens
        .insert("t5".into(), format!("a\nb\nc\nd\n{long} touch x"));
    let view = looked(&mut rig);
    let lines = view["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "c");
    assert!(lines[2].as_str().unwrap().ends_with("$ touch x"));
    assert!(view["text"].as_str().unwrap().starts_with("a\n"));
}

#[test]
fn a_moved_screen_is_watched_until_it_stops_so_the_prompt_is_the_last_one() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.world()
        .screens
        .insert("t5".into(), "banner\nrig$ ".into());
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Waiting, "still drawing, maybe");
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("screen changed"));
    let view = looked(&mut rig);
    assert!(view.is_object());
    confirm(&mut rig, &view);
    assert_eq!(rig.sends().len(), 1, "a staged rc can still be resumed");
}

#[test]
fn someone_typing_gets_no_view_and_so_no_send() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    rig.world().quiet = false;
    let view = looked(&mut rig);
    assert_eq!(view, Value::Null);
    assert!(rig.reason().contains("typing"));
    assert!(rig.sends().is_empty());
}

#[test]
fn clicks_that_are_stale_early_or_for_another_row_do_nothing() {
    let mut rig = Rig::new();
    let before = clock();
    pass(10);
    losing_without_reboot(&mut rig);
    let loss = rig.saved().loss.unwrap().detected_ms;
    let looks = |rig: &mut Rig| {
        rig.tick(&[]);
        rig.saved().loss.unwrap().entries["t5"].look.clone()
    };
    press(&rig, loss, None, before);
    assert_eq!(looks(&mut rig), None, "from before the loss");
    press(&rig, loss + 1, None, clock());
    assert_eq!(looks(&mut rig), None, "for another loss");
    press(&rig, loss, None, clock() + 60_000);
    assert_eq!(looks(&mut rig), None, "stamped in the future");
    let at = clock();
    pass(60_001);
    press(&rig, loss, None, at);
    assert_eq!(looks(&mut rig), None, "too old: nobody may be there");
    press(&rig, loss, Some("wrong".into()), clock());
    rig.tick(&[]);
    assert!(rig.sends().is_empty());
}

#[test]
fn a_click_left_in_the_store_is_acted_on_once() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    let view = looked(&mut rig);
    assert_eq!(view["lines"], json!(["$"]));
    rig.world().screens.insert("t5".into(), "$ later".into());
    rig.tick(&[]);
    let look = rig.saved().loss.unwrap().entries["t5"]
        .look
        .clone()
        .unwrap();
    assert_eq!(look.lines, ["$"], "the same click did not look again");
}

#[test]
fn a_view_expires_and_a_token_must_match() {
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    let view = looked(&mut rig);
    confirm(&mut rig, &json!({"token": "wrong"}));
    assert!(rig.sends().is_empty());
    assert!(rig.reason().contains("look again"));
    let view2 = looked(&mut rig);
    assert_eq!(view["token"], view2["token"]);
    pass(60_001);
    rig.tick(&[]);
    assert_eq!(rig.saved().loss.unwrap().entries["t5"].look, None);
    confirm(&mut rig, &view2);
    assert!(rig.sends().is_empty());
}

#[test]
fn a_click_keeps_every_terminal_gate() {
    let refused: Vec<(&str, Term, &str)> = vec![
        (
            "program",
            Term {
                foreground: Some(fg(88, CWD)),
                ..shell_term()
            },
            "running",
        ),
        ("exec", exec("python3", &["python3"]), "isn't a shell"),
        (
            "folder",
            Term {
                foreground: Some(fg(77, "/fixture/other")),
                ..shell_term()
            },
            "folder",
        ),
    ];
    for (name, term, reason) in refused.clone() {
        let mut rig = Rig::new();
        losing_without_reboot(&mut rig);
        rig.set(term);
        assert_eq!(looked(&mut rig), Value::Null, "{name}");
        assert!(rig.reason().contains(reason), "{name}: {}", rig.reason());
    }
    // And again between the two clicks.
    for (name, term, reason) in refused {
        let mut rig = Rig::new();
        losing_without_reboot(&mut rig);
        let view = looked(&mut rig);
        rig.set(term);
        confirm(&mut rig, &view);
        assert!(rig.sends().is_empty(), "{name}");
        assert!(rig.reason().contains(reason), "{name}: {}", rig.reason());
    }
}

#[test]
fn a_button_is_only_offered_while_it_can_work() {
    let mut rig = Rig::new();
    rig.lost(exec("python3", &["python3"]));
    let report = rig.tick(&[]).unwrap();
    assert_eq!(report["loss"]["entries"][0]["click"], false, "not a shell");
    let mut rig = Rig::new();
    rig.lost(Term {
        live: false,
        ..shell_term()
    });
    let report = rig.tick(&[]).unwrap();
    assert_eq!(report["loss"]["entries"][0]["click"], false, "finished");
    // Found dead by a click: the button goes.
    let mut rig = Rig::new();
    losing_without_reboot(&mut rig);
    rig.set(exec("python3", &["python3"]));
    looked(&mut rig);
    let report = rig.tick(&[]).unwrap();
    assert_eq!(report["loss"]["entries"][0]["click"], false);
}

#[test]
fn a_foreground_child_at_startup_is_waited_for_and_ends_with_a_working_button() {
    let mut rig = Rig::new();
    rig.lost(Term {
        foreground: Some(fg(88, CWD)),
        ..shell_term()
    });
    assert_eq!(
        rig.stage(),
        Stage::Waiting,
        "an rc file's child is not a person"
    );
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty(), "the first screen is unknown");
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("before its shell was ready"));
    let view = looked(&mut rig);
    confirm(&mut rig, &view);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_row_that_never_saw_a_prompt_can_still_be_confirmed() {
    let mut rig = Rig::new();
    rig.lost(Term {
        foreground: Some(fg(88, CWD)),
        ..shell_term()
    });
    pass(SHELL_WAIT_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    rig.set(shell_term());
    let view = looked(&mut rig);
    assert!(view.is_object());
    confirm(&mut rig, &view);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_program_after_the_first_prompt_is_a_final_skip_with_a_button() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.set(Term {
        foreground: Some(fg(88, CWD)),
        ..shell_term()
    });
    let report = rig.tick(&[]).unwrap();
    assert_eq!(rig.stage(), Stage::Skipped);
    assert_eq!(report["loss"]["entries"][0]["click"], true);
}

#[test]
fn a_missing_saved_boot_id_never_types_by_itself() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    let mut saved = rig.saved();
    saved.boot = None;
    let path = rig.home.path.join(GEN).join(store::FILE);
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    next.saved = Some(saved);
    next.started = true;
    next.store.load(&rig.home, ID).unwrap();
    rig.reboot("holder-b");
    next.tick(&rig.home, &[], &|| true);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    next.tick(&rig.home, &[], &|| true);
    pass(SETTLE_MS);
    next.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn a_failed_boot_read_keeps_the_saved_id() {
    fn none() -> Option<String> {
        None
    }
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    let id = rig.saved().boot;
    assert!(id.is_some());
    rig.resume.boot = none;
    rig.tick(&[link("t5", OTHER, &["claude"])]);
    assert_eq!(rig.saved().boot, id);
}

#[test]
fn without_a_reboot_the_same_pid_means_nothing_was_lost() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().status = Some(status("holder-b"));
    rig.set(Term {
        reports: false,
        foreground: Some(fg(20, CWD)),
        ..agent_term()
    });
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("never replaced"));
}

#[test]
fn after_a_reboot_a_repeated_pid_means_nothing() {
    let mut rig = Rig::new();
    rig.lost(Term {
        pid: Some(20),
        foreground: Some(fg(20, CWD)),
        ..shell_term()
    });
    rig.settle();
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_blank_read_is_a_witness_too() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // The shell is up before Panorama looks, but it hasn't drawn yet.
    BOOT.with(|b| b.set(b.get() + 1));
    rig.world().status = Some(status("holder-b"));
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), " \n".into());
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.tick(&[]);
    rig.settle();
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn another_config_dir_is_shown_not_typed() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    let mut other = link("t5", SESSION, &["claude"]);
    other.default_config = false;
    rig.tick(&[other]);
    rig.reboot("holder-b");
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert!(rig.reason().contains("config dir"));
}

#[test]
fn leaving_after_the_toggle_went_off_leaves_no_record() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    std::fs::remove_file(rig.home.path.join(TOGGLE_STORE)).unwrap();
    rig.resume.close(&rig.home);
    assert!(!rig.home.path.join(GEN).join(store::FILE).exists());
}

#[test]
fn a_long_up_core_may_have_used_shells() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.reboot("holder-b");
    rig.world().status = Some(Status {
        uptime_ms: FRESH_CORE_MS + 1,
        ..status("holder-b")
    });
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn an_unproved_session_is_shown_not_typed() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // Same process, but the link no longer holds: an in-process switch may be under way.
    rig.tick(&[]);
    assert!(!rig.saved().records["t5"].proved);
    rig.reboot("holder-b");
    rig.tick(&[]);
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn only_local_uuid_sessions_of_known_harnesses_are_recorded() {
    let bad = [
        link("vps/t5", SESSION, &["claude"]),
        link("t5", "$(rm -rf x)", &["claude"]),
        Linked {
            harness: "gemini".into(),
            ..link("t5", SESSION, &["gemini"])
        },
    ];
    for l in &bad {
        assert_eq!(decide::record(l), None, "{l:?}");
    }
    let mut rig = Rig::new();
    rig.tick(&bad);
    assert!(rig.saved().records.is_empty());
}

#[test]
fn a_tampered_record_cannot_put_text_in_a_shell() {
    let record = Record {
        session: "x; rm -rf ~".into(),
        ..decide::record(&link("t5", SESSION, &["claude"])).unwrap()
    };
    assert_eq!(record.command(), None);
}

#[test]
fn each_harness_gets_its_own_command() {
    let of = |harness: &str, argv: &[&str]| {
        let l = Linked {
            harness: harness.into(),
            ..link("t5", SESSION, argv)
        };
        let r = decide::record(&l).unwrap();
        (r.command().unwrap(), r.launch)
    };
    let id = SESSION;
    let plain = |command: String| (command, Launch::Plain);
    let codex = format!("codex resume {id}");
    let daemonless = format!("codex --no-daemon resume {id}");
    assert_eq!(
        of("claude", &["claude"]),
        plain(format!("claude --resume {id}"))
    );
    assert_eq!(of("codex", &["codex"]), plain(codex.clone()));
    assert_eq!(of("codex", &["codex", "resume", id]), plain(codex));
    assert_eq!(
        of("codex", &["codex", "--no-daemon"]),
        plain(daemonless.clone())
    );
    let late = of("codex", &["codex", "--model", "x", "--no-daemon"]);
    assert_eq!(late, plain(daemonless.clone()), "found anywhere (L5)");
    let resumed = of("codex", &["codex", "--no-daemon", "resume", id]);
    assert_eq!(resumed, plain(daemonless));
    assert_eq!(of("codex", &["codex", "exec", "x"]).1, Launch::Other);
    assert_eq!(of("codex", &["codex", "--search"]).1, Launch::Other);
    assert_eq!(of("codex", &["codex", "a task"]).1, Launch::Other);
}

#[test]
fn a_failed_send_is_read_and_never_retried() {
    for failure in [
        Sent::Failed(None),
        Sent::Failed(Some("status_mismatch".into())),
    ] {
        let mut rig = Rig::new();
        rig.world().send = Some(failure);
        rig.lost(shell_term());
        rig.settle();
        assert_eq!(rig.sends().len(), 1);
        assert_eq!(rig.stage(), Stage::Sent);
        assert!(rig.reason().contains("send failed"));
        rig.tick(&[]);
        pass(CONFIRM_MS);
        rig.tick(&[]);
        pass(CONFIRM_MS);
        rig.tick(&[]);
        assert_eq!(rig.sends().len(), 1);
        assert_eq!(rig.stage(), Stage::Failed);
        assert!(rig.reason().contains("not retried"));
    }
}

#[test]
fn a_failed_send_that_did_arrive_counts_as_arrived() {
    let mut rig = Rig::new();
    rig.world().send = Some(Sent::Failed(None));
    rig.lost(shell_term());
    rig.settle();
    // It got there after all, and the program started.
    rig.set(Term {
        foreground: Some(fg(90, CWD)),
        ..shell_term()
    });
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    assert_eq!(rig.stage(), Stage::Resumed);
    assert_eq!(rig.sends().len(), 1);
    let mut rig = Rig::new();
    rig.world().send = Some(Sent::Failed(None));
    rig.lost(shell_term());
    rig.settle();
    let typed = format!("$ claude --resume {SESSION}\nnot found");
    rig.world().screens.insert("t5".into(), typed);
    pass(CONFIRM_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Failed);
    assert!(rig.reason().contains("reached the terminal"));
}

#[test]
fn a_restart_mid_send_never_sends_again() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    // The writer died between saving the attempt and finishing: Sending is on disk.
    let mut saved = rig.saved();
    saved
        .loss
        .as_mut()
        .unwrap()
        .entries
        .get_mut("t5")
        .unwrap()
        .stage = Stage::Sending;
    let path = rig.home.path.join(GEN).join(store::FILE);
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    next.tick(&rig.home, &[], &|| true);
    pass(SETTLE_MS);
    next.tick(&rig.home, &[], &|| true);
    pass(CONFIRM_MS);
    next.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Failed);
}

#[test]
fn the_attempt_is_on_disk_before_the_text_is_typed() {
    struct Check(Fake, PathBuf);
    impl Hc for Check {
        fn status(&self) -> Option<Status> {
            self.0.status()
        }
        fn get(&self, a: &str) -> Lookup {
            self.0.get(a)
        }
        fn screen(&self, a: &str) -> Option<String> {
            self.0.screen(a)
        }
        fn quiet(&self, a: &str) -> bool {
            self.0.quiet(a)
        }
        fn send(&self, a: &str, t: &str) -> Sent {
            let saved: Saved = serde_json::from_slice(&std::fs::read(&self.1).unwrap()).unwrap();
            assert_eq!(saved.loss.unwrap().entries["t5"].stage, Stage::Sending);
            self.0.send(a, t)
        }
    }
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS);
    let path = rig.home.path.join(GEN).join(store::FILE);
    let mut next = Resume::with(
        Check(rig.fake.clone(), path),
        ID,
        GEN,
        rig.locks.clone(),
        CLOCKS,
    );
    rig.resume.held = None;
    next.saved = rig.resume.saved.clone();
    next.started = true;
    next.store.load(&rig.home, ID).unwrap();
    next.settled = rig.resume.settled.clone();
    next.tick(&rig.home, &[], &|| true);
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn an_attempt_that_cannot_be_saved_is_not_made() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    // A directory where the staging file goes: the save fails, so nothing may be typed.
    std::fs::create_dir(rig.home.path.join(GEN).join(store::TEMP)).unwrap();
    let report = rig.settle().unwrap();
    assert!(rig.sends().is_empty());
    assert!(report["error"].is_string());
    assert_eq!(report["loss"]["entries"][0]["state"], "skipped");
}

#[test]
fn a_resumed_process_dying_soon_after_keeps_its_id_visible() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.settle();
    rig.set(Term {
        foreground: Some(fg(90, CWD)),
        ..shell_term()
    });
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Sent);
    rig.set(shell_term());
    let report = rig.tick(&[]).unwrap();
    assert_eq!(rig.stage(), Stage::Failed);
    let entry = &report["loss"]["entries"][0];
    assert_eq!(entry["session_id"], SESSION);
    assert_eq!(entry["command"], format!("claude --resume {SESSION}"));
    pass(CONFIRM_MS);
    rig.tick(&[]);
    assert_eq!(rig.sends().len(), 1, "herdr#5532: never retried");
}

#[test]
fn a_started_program_without_a_link_is_reported_as_unproved() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.settle();
    rig.set(Term {
        foreground: Some(fg(90, CWD)),
        ..shell_term()
    });
    rig.tick(&[]);
    pass(CONFIRM_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Resumed);
    assert!(rig.reason().contains("isn't proved"));
}

#[test]
fn a_resume_under_a_new_id_says_so() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.settle();
    rig.tick(&[link("t5", OTHER, &["claude"])]);
    assert_eq!(rig.stage(), Stage::Resumed);
    assert!(rig.reason().contains(OTHER));
}

#[test]
fn a_second_loss_starts_fresh_and_closes_what_was_open() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    assert_eq!(rig.stage(), Stage::Waiting);
    rig.reboot("holder-c");
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("another loss"));
    rig.settle();
    assert!(rig.sends().is_empty());
    // A session proved after that is a new attempt on the next loss.
    rig.lost(shell_term());
    rig.settle();
    assert_eq!(rig.sends().len(), 1);
}

#[test]
fn a_later_loss_keeps_what_could_not_be_resumed_and_drops_what_was() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.settle();
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    assert_eq!(rig.stage(), Stage::Resumed);
    let old = rig.saved().loss.unwrap().detected_ms;
    // A loss with nothing recorded changes nothing that was reported.
    rig.world().terms.clear();
    rig.tick(&[]);
    pass(ENDING_MS);
    rig.tick(&[]);
    assert!(rig.saved().records.is_empty());
    pass(1000);
    rig.reboot("holder-c");
    rig.tick(&[]);
    assert_eq!(rig.saved().loss.as_ref().unwrap().detected_ms, old);
    // A skipped id survives the next loss; a resumed one does not.
    let mut rig = Rig::new();
    rig.lost(Term {
        foreground: Some(fg(77, "/fixture/other")),
        ..shell_term()
    });
    assert_eq!(rig.stage(), Stage::Skipped);
    let mut other = agent_term();
    other.created_ms = 7;
    rig.world().terms.insert("t6".into(), Lookup::Found(other));
    let mut second = link("t6", OTHER, &["claude"]);
    second.terminal["created_ms"] = json!(7);
    rig.tick(&[second]);
    rig.world().status = Some(status("holder-c"));
    rig.tick(&[]);
    let entries = rig.saved().loss.unwrap().entries;
    assert_eq!(entries["t5"].stage, Stage::Skipped);
    assert_eq!(entries["t6"].record.session, OTHER);
}

#[test]
fn a_wiped_core_invalidates_every_address() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().status = Some(Status {
        core: "beef".into(),
        ..status("holder-b")
    });
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    rig.tick(&[]);
    rig.settle();
    assert!(rig.sends().is_empty());
    assert!(rig.saved().records.is_empty());
}

#[test]
fn two_writers_on_one_core_cannot_both_act() {
    let mut rig = Rig::new();
    rig.lost(shell_term());
    // A second app home on the same core, with its own record of the same terminal.
    let other = Rig::new();
    let fake = rig.fake.clone();
    let mut second = Resume::with(fake, ID, GEN, rig.locks.clone(), CLOCKS);
    let report = second.tick(&other.home, &[], &|| true).unwrap();
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("another Panorama")
    );
    assert!(!other.home.path.join(GEN).join(store::FILE).exists());
    rig.settle();
    assert_eq!(rig.sends().len(), 1, "the first still acts, once");
}

#[test]
fn a_corrupt_record_types_nothing() {
    let mut rig = Rig::new();
    let path = rig.home.path.join(GEN).join(store::FILE);
    std::fs::write(&path, "{broken").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    rig.world().screens.insert("t5".into(), "$ ".into());
    rig.set(shell_term());
    let report = rig.tick(&[]).unwrap();
    assert!(report["error"].is_string(), "the reason is shown");
    rig.settle();
    assert!(rig.sends().is_empty());
    assert_eq!(
        rig.world().calls,
        0,
        "no hc call on a record it cannot read"
    );
}

#[test]
fn a_record_from_another_installation_is_refused() {
    let mut rig = Rig::new();
    rig.tick(&[]);
    let path = rig.home.path.join(GEN).join(store::FILE);
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace(ID, &"f".repeat(32));
    std::fs::write(&path, text).unwrap();
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    let report = next.tick(&rig.home, &[], &|| true).unwrap();
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("another installation")
    );
}

#[test]
fn the_record_survives_as_one_private_file_and_clears_a_crashed_staging_file() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    let dir = rig.home.path.join(GEN);
    let mode = std::fs::metadata(dir.join(store::FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let record = &rig.saved().records["t5"];
    assert_eq!(
        (
            record.address.as_str(),
            record.harness.as_str(),
            record.session.as_str(),
            record.cwd.as_str()
        ),
        ("t5", "claude", SESSION, CWD)
    );
    std::fs::write(dir.join(store::TEMP), "partial").unwrap();
    std::fs::set_permissions(
        dir.join(store::TEMP),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), CLOCKS);
    rig.resume.held = None;
    assert_eq!(
        next.tick(&rig.home, &[], &|| true).unwrap()["error"],
        Value::Null
    );
    assert!(!dir.join(store::TEMP).exists());
}

#[test]
fn busy_holds_only_while_something_is_open() {
    let mut rig = Rig::new();
    assert!(!rig.resume.busy());
    rig.lost(shell_term());
    assert!(rig.resume.busy());
    rig.settle();
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    assert!(!rig.resume.busy());
}
