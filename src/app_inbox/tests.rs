use super::*;
use crate::panel::act::inbox::{INBOX_ID, OPEN_ACTION, Ran, Runner};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use std::{cell::RefCell, os::unix::fs::PermissionsExt};
struct Script {
    calls: RefCell<Vec<Vec<String>>>,
    code: i32,
    help: &'static str,
}
impl Runner for Script {
    async fn inbox(&self, _: &str, args: &[&str]) -> Ran {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| s.to_string()).collect());
        Ran {
            code: if args == ["open", "--help"] {
                0
            } else {
                self.code
            },
            out: self.help.into(),
        }
    }
}
fn fake() -> FakeHerdr {
    let h = FakeHerdr::start();
    h.on("plugin.list",Reply::Result(json!({"plugins":[{"plugin_id":INBOX_ID,"plugin_root":"/fixture","enabled":true,"version":"0.10.5"}]})));
    h
}
#[tokio::test(flavor = "current_thread")]
async fn installed_navigation_is_exact_and_never_falls_back_to_an_action_or_answer() {
    let h = fake();
    let link = InboxLink::default();
    let run = Script {
        calls: RefCell::default(),
        code: 0,
        help: "usage: open --question --no-hold",
    };
    let installed = link.installed(&h.client(), &run).await.unwrap().unwrap();
    assert!(installed.questions);
    installed.open(&run, Some(14)).await.unwrap();
    installed.open(&run, None).await.unwrap();
    assert_eq!(
        *run.calls.borrow(),
        vec![
            vec!["open", "--help"],
            vec!["open", "--question", "Q14", "--no-hold"],
            vec!["open", "--no-hold"]
        ]
    );
    assert!(
        h.params_of("plugin.action.invoke").is_empty(),
        "{OPEN_ACTION}"
    );
    assert_eq!(
        h.calls()
            .iter()
            .map(|(m, _)| m.as_str())
            .collect::<Vec<_>>(),
        vec!["plugin.list"]
    );
    let failed = Script {
        calls: RefCell::default(),
        code: 3,
        help: "",
    };
    assert!(installed.open(&failed, Some(14)).await.is_err());
    assert_eq!(failed.calls.borrow().len(), 1);
    assert!(h.params_of("plugin.action.invoke").is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn absent_question_capability_does_not_open_plain_instead() {
    let h = fake();
    let run = Script {
        calls: RefCell::default(),
        code: 0,
        help: "usage: open",
    };
    let i = InboxLink::default()
        .installed(&h.client(), &run)
        .await
        .unwrap()
        .unwrap();
    assert!(!i.questions);
    assert!(i.open(&run, Some(1)).await.is_err());
    assert_eq!(run.calls.borrow().len(), 1);
}
#[test]
fn grammar_rejects_unsafe_ids_and_non_navigation_before_any_io() {
    for words in [
        "--open --question 0 --app-home /missing",
        "--open --question 01 --app-home /missing",
        "--open --question Q14 --app-home /missing",
        "--open --question 9007199254740992 --app-home /missing",
        "--available --question 14 --app-home /missing",
        "--open --answer yes --app-home /missing",
        "--open --app-home relative",
    ] {
        assert!(
            arguments(
                &words
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            )
            .is_err(),
            "{words}"
        );
    }
}
#[test]
fn installed_command_strips_callers_and_python_writes_and_rejects_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join(".venv/bin/inbox");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let keys = [
        "HC_TERMINAL_ID",
        "HC_CALLER",
        "HERDR_PANE_ID",
        "HERDR_PLUGIN_CONTEXT_JSON",
        "HERDR_PLUGIN_ID",
        "HERDR_SOCKET_PATH",
        "UV_PROJECT",
        "PYTHONPATH",
        "PYTHONDONTWRITEBYTECODE",
        "INBOX_OPEN_NONCE",
    ];
    let cmd = runner::command_for(
        dir.path().to_str().unwrap(),
        &["open", "--question", "Q14"],
        keys.into_iter().map(str::to_owned),
    )
    .unwrap();
    assert_eq!(cmd.get_program(), bin.as_os_str());
    let env: std::collections::HashMap<_, _> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|s| s.to_string_lossy().into_owned()),
            )
        })
        .collect();
    for key in [
        "HC_TERMINAL_ID",
        "HC_CALLER",
        "HERDR_PANE_ID",
        "HERDR_PLUGIN_CONTEXT_JSON",
        "UV_PROJECT",
        "PYTHONPATH",
        "INBOX_OPEN_NONCE",
    ] {
        assert_eq!(env.get(key), Some(&None), "{key}");
    }
    assert_eq!(env["PYTHONDONTWRITEBYTECODE"], Some("1".into()));
    assert!(!env.contains_key("HERDR_SOCKET_PATH"));
    assert!(
        runner::command_for(dir.path().to_str().unwrap(), &["answer", "Q14", "yes"], []).is_err()
    );
    std::fs::remove_file(&bin).unwrap();
    std::os::unix::fs::symlink("/bin/sh", &bin).unwrap();
    assert!(runner::command_for(dir.path().to_str().unwrap(), &["open"], []).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn a_help_timeout_is_an_error_instead_of_missing_capability() {
    struct TimedOut;
    impl Runner for TimedOut {
        async fn inbox(&self, _: &str, _: &[&str]) -> Ran {
            Ran {
                code: 124,
                out: "timeout".into(),
            }
        }
    }
    let h = fake();
    assert!(
        InboxLink::default()
            .installed(&h.client(), &TimedOut)
            .await
            .is_err()
    );
    assert_eq!(h.calls().len(), 1);
}
