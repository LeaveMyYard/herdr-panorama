//! A scripted hc world: every "must not type" case asserts on the sends it recorded.
use super::{
    decide::{self, CONFIRM_MS, ENDING_MS, FRESH_CORE_MS, SETTLE_MS, SHELL_WAIT_MS},
    hc::{Foreground, Hc, Lookup, Sent, Status, Term},
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

thread_local! { static NOW: Cell<u64> = const { Cell::new(1_000_000) }; }
fn clock() -> u64 {
    NOW.with(Cell::get)
}
fn pass(ms: u64) {
    NOW.with(|n| n.set(n.get() + ms));
}

#[derive(Default)]
struct World {
    status: Option<Status>,
    terms: BTreeMap<String, Lookup>,
    screens: BTreeMap<String, String>,
    send: Option<Sent>,
    sends: Vec<(String, String)>,
    calls: usize,
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
        reports: true,
        foreground: Some(Foreground {
            pid: 30,
            cwd: CWD.into(),
        }),
    }
}
/// The recovery shell: a new program, in front of nothing.
fn shell_term() -> Term {
    Term {
        pid: Some(77),
        reports: false,
        foreground: Some(Foreground {
            pid: 77,
            cwd: CWD.into(),
        }),
        ..agent_term()
    }
}
fn link(address: &str, session: &str, argv: &[&str]) -> Linked {
    Linked {
        address: address.into(),
        harness: "claude".into(),
        session: session.into(),
        terminal: json!({"state":"live","keep":true,"pid":20,"created_ms":5,
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
        let resume = Resume::with(fake.clone(), ID, GEN, locks.clone(), clock);
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
    /// A watched agent, then the loss, with this terminal back.
    fn lost(&mut self, term: Term) -> Option<Value> {
        self.set(agent_term());
        self.tick(&[link("t5", SESSION, &["claude"])]);
        self.world().status = Some(status("holder-b"));
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
        foreground: Some(Foreground {
            pid: 90,
            cwd: CWD.into(),
        }),
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
    rig.world().status = Some(status("holder-b"));
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
        fn send(&self, a: &str, t: &str) -> Sent {
            self.0.send(a, t)
        }
    }
    let mut flip = Resume::with(
        Flip(rig.fake.clone(), home),
        ID,
        GEN,
        rig.locks.clone(),
        clock,
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
    rig.resume.held = None;
    let shell = Term {
        pid: Some(20),
        reports: false,
        foreground: Some(Foreground {
            pid: 20,
            cwd: CWD.into(),
        }),
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
        foreground: Some(Foreground {
            pid: 20,
            cwd: CWD.into(),
        }),
        ..agent_term()
    });
    rig.tick(&[]);
    pass(ENDING_MS);
    rig.tick(&[]);
    rig.world().status = Some(status("holder-b"));
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
        foreground: Some(Foreground {
            pid: 20,
            cwd: CWD.into(),
        }),
        ..agent_term()
    });
    rig.tick(&[]);
    rig.world().status = Some(status("holder-b"));
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
        foreground: Some(Foreground {
            pid: 20,
            cwd: CWD.into(),
        }),
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
    rig.resume.held = None;
    rig.world().status = Some(status("holder-b"));
    rig.set(shell_term());
    rig.world().screens.insert("t5".into(), "$ ".into());
    let report = next.tick(&rig.home, &[], &|| true).unwrap();
    pass(SETTLE_MS);
    next.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
    // It can still be resumed by hand: the report says how.
    let entry = &report["loss"]["entries"][0];
    assert_eq!(entry["state"], "skipped");
    assert_eq!(entry["command"], format!("claude --resume {SESSION}"));
}

#[test]
fn a_killed_writer_still_proves_a_reboot() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    // No close: the machine went down with it. The app opens first, then the shell returns.
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
    rig.resume.held = None;
    rig.world().status = Some(Status {
        holders: vec![],
        ..status("x")
    });
    rig.set(Term {
        pid: None,
        reason: Some("stopped_by_restart".into()),
        foreground: None,
        ..agent_term()
    });
    next.tick(&rig.home, &[], &|| true);
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
    rig.resume.held = None;
    rig.world().terms.insert("t5".into(), Lookup::Unknown);
    next.tick(&rig.home, &[], &|| true);
    assert!(!rig.saved().records["t5"].proved);
}

#[test]
fn every_busy_or_doubtful_terminal_is_shown_and_never_typed_into() {
    let running = Some(Foreground {
        pid: 88,
        cwd: CWD.into(),
    });
    let cases: Vec<(&str, Term, &str)> = vec![
        (
            "foreground program",
            Term {
                foreground: running,
                ..shell_term()
            },
            "running",
        ),
        (
            "reports status",
            Term {
                reports: true,
                ..shell_term()
            },
            "reporting",
        ),
        (
            "wrong folder",
            Term {
                foreground: Some(Foreground {
                    pid: 77,
                    cwd: "/fixture/other".into(),
                }),
                ..shell_term()
            },
            "folder",
        ),
        (
            "not replaced",
            Term {
                pid: Some(20),
                foreground: Some(Foreground {
                    pid: 20,
                    cwd: CWD.into(),
                }),
                ..shell_term()
            },
            "never replaced",
        ),
        (
            "different terminal",
            Term {
                created_ms: 6,
                ..shell_term()
            },
            "different terminal",
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
    rig.world().status = Some(status("holder-b"));
    rig.world().terms.clear();
    rig.tick(&[]);
    assert!(rig.sends().is_empty());
    assert_eq!(rig.stage(), Stage::Skipped);
}

#[test]
fn a_shell_that_has_not_settled_is_not_typed_into() {
    // A blank screen: nothing drawn yet.
    let mut rig = Rig::new();
    rig.lost(shell_term());
    rig.world().screens.insert("t5".into(), "   \n".into());
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
    // A screen still changing: startup output, or someone typing.
    for n in 0..4 {
        rig.world().screens.insert("t5".into(), format!("$ {n}"));
        rig.settle();
    }
    assert!(rig.sends().is_empty());
    // Past the give-up nothing is typed, however quiet it becomes.
    pass(SHELL_WAIT_MS);
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    rig.settle();
    rig.settle();
    assert!(rig.sends().is_empty());
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
                foreground: Some(Foreground {
                    pid: 88,
                    cwd: CWD.into(),
                }),
                ..shell_term()
            });
            self.0.0.borrow_mut().terms.insert("t5".into(), found);
            self.0.screen(a)
        }
        fn send(&self, a: &str, t: &str) -> Sent {
            self.0.send(a, t)
        }
    }
    let mut rig = Rig::new();
    rig.lost(shell_term());
    pass(SETTLE_MS);
    let mut late = Resume::with(Late(rig.fake.clone()), ID, GEN, rig.locks.clone(), clock);
    rig.resume.held = None;
    late.saved = rig.resume.saved.clone();
    late.started = true;
    late.store.load(&rig.home, ID).unwrap();
    late.settled = rig.resume.settled.clone();
    late.tick(&rig.home, &[], &|| true);
    assert!(rig.sends().is_empty());
}

#[test]
fn modes_the_command_cannot_repeat_are_shown_not_typed() {
    for argv in [
        &["claude", "rc"][..],
        &["claude", "--verbose", "remote-control"][..],
    ] {
        let mut rig = Rig::new();
        rig.set(agent_term());
        rig.tick(&[link("t5", SESSION, argv)]);
        rig.world().status = Some(status("holder-b"));
        rig.world().screens.insert("t5".into(), "$ ".into());
        rig.set(shell_term());
        rig.tick(&[]);
        rig.settle();
        assert!(rig.sends().is_empty(), "{argv:?}");
        assert!(rig.reason().contains("mode"));
    }
}

#[test]
fn a_long_up_core_may_have_used_shells() {
    let mut rig = Rig::new();
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().status = Some(Status {
        uptime_ms: FRESH_CORE_MS + 1,
        ..status("holder-b")
    });
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
    rig.world().status = Some(status("holder-b"));
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
    assert_eq!(
        of("claude", &["claude"]),
        (format!("claude --resume {id}"), Launch::Plain)
    );
    assert_eq!(
        of("codex", &["codex"]),
        (format!("codex resume {id}"), Launch::Plain)
    );
    assert_eq!(
        of("codex", &["codex", "resume", id]),
        (format!("codex resume {id}"), Launch::Plain)
    );
    assert_eq!(
        of("codex", &["codex", "--no-daemon"]),
        (format!("codex --no-daemon resume {id}"), Launch::NoDaemon)
    );
    assert_eq!(of("codex", &["codex", "exec", "x"]).1, Launch::Other);
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
        foreground: Some(Foreground {
            pid: 90,
            cwd: CWD.into(),
        }),
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
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
        clock,
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
        foreground: Some(Foreground {
            pid: 90,
            cwd: CWD.into(),
        }),
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
        foreground: Some(Foreground {
            pid: 90,
            cwd: CWD.into(),
        }),
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
    rig.world().status = Some(status("holder-c"));
    rig.tick(&[]);
    assert_eq!(rig.stage(), Stage::Skipped);
    assert!(rig.reason().contains("another loss"));
    rig.settle();
    assert!(rig.sends().is_empty());
    // A session proved after that is a new attempt on the next loss.
    rig.set(agent_term());
    rig.tick(&[link("t5", SESSION, &["claude"])]);
    rig.world().status = Some(status("holder-d"));
    rig.set(shell_term());
    rig.tick(&[]);
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
    rig.world().status = Some(status("holder-c"));
    rig.tick(&[]);
    assert_eq!(rig.saved().loss.as_ref().unwrap().detected_ms, old);
    // A skipped id survives the next loss; a resumed one does not.
    let mut rig = Rig::new();
    rig.lost(Term {
        foreground: Some(Foreground {
            pid: 88,
            cwd: CWD.into(),
        }),
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
    let mut second = Resume::with(fake, ID, GEN, rig.locks.clone(), clock);
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
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
    let mut next = Resume::with(rig.fake.clone(), ID, GEN, rig.locks.clone(), clock);
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
