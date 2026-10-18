//! The inbox link (Python tests/test_inbox_link.py's cases, ported).

use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;
use std::cell::RefCell;

/// A scripted inbox CLI: `help` for `open --help`, `open` for the deep link.
struct Inbox {
    help: Ran,
    open: Ran,
    calls: RefCell<Vec<Vec<String>>>,
}

impl Runner for Inbox {
    async fn inbox(&self, _root: &str, args: &[&str]) -> Ran {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|a| (*a).to_owned()).collect());
        if args.contains(&"--help") {
            self.help.clone()
        } else {
            self.open.clone()
        }
    }
}

fn inbox(help: &str, open: (i32, &str)) -> Inbox {
    Inbox {
        help: Ran {
            code: 0,
            out: help.into(),
        },
        open: Ran {
            code: open.0,
            out: open.1.into(),
        },
        calls: RefCell::default(),
    }
}

fn listed(version: &str) -> FakeHerdr {
    let fake = FakeHerdr::start();
    fake.on(
        "plugin.list",
        Reply::Result(json!({"plugins": [
        {"plugin_id": INBOX_ID, "version": version, "plugin_root": "/inbox", "enabled": true}]})),
    );
    fake.on("plugin.action.invoke", Reply::Result(json!({})));
    fake
}

fn plain(fake: &FakeHerdr) -> usize {
    fake.params_of("plugin.action.invoke").len()
}

#[tokio::test(flavor = "current_thread")]
async fn a_question_opens_its_group_through_the_deep_link() {
    let fake = listed("0.10.5");
    let run = inbox("usage: --pane --question --no-hold", (0, ""));
    assert_eq!(
        InboxLink::default()
            .open(&fake.client(), &run, Some(7), None)
            .await,
        None
    );
    assert_eq!(
        run.calls.borrow().last().unwrap(),
        &["open", "--question", "Q7", "--no-hold"]
    );
    assert_eq!(plain(&fake), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn no_hold_only_when_the_help_lists_it() {
    let fake = listed("0.10.5");
    let run = inbox("usage: --pane --question", (0, ""));
    InboxLink::default()
        .open(&fake.client(), &run, None, Some("w1:p1"))
        .await;
    assert_eq!(
        run.calls.borrow().last().unwrap(),
        &["open", "--pane", "w1:p1"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_deep_link_that_failed_before_the_popup_falls_back_to_the_plain_one() {
    let fake = listed("0.10.5");
    let run = inbox("--pane", (2, "error: unrecognized arguments: --pane"));
    assert_eq!(
        InboxLink::default()
            .open(&fake.client(), &run, None, Some("w1:p1"))
            .await,
        None
    );
    assert_eq!(
        fake.params_of("plugin.action.invoke"),
        [json!({"action_id": OPEN_ACTION, "context": {"invocation_source": "cli"}})]
    );
    // a failure after the popup may have opened: no second popup
    let fake = listed("0.10.5");
    let run = inbox("--pane", (1, "boom"));
    let err = InboxLink::default()
        .open(&fake.client(), &run, None, Some("w1:p1"))
        .await;
    assert_eq!(
        err.as_deref(),
        Some("inbox open failed (exit 1); no second popup")
    );
    assert_eq!(plain(&fake), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn an_old_inbox_opens_plain() {
    // older than 0.6.0 and its help has no --pane: the plain action
    let fake = listed("0.5.9");
    let run = inbox("usage: open", (0, ""));
    InboxLink::default()
        .open(&fake.client(), &run, Some(1), None)
        .await;
    assert_eq!(plain(&fake), 1);
    assert!(
        !run.calls
            .borrow()
            .iter()
            .any(|c| c.contains(&"--question".to_owned()))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn not_installed_or_an_empty_pane_runs_the_plain_action() {
    // test_not_installed_or_failing_deep_link_falls_back: the caller only asks with inbox_ok
    let fake = FakeHerdr::start();
    fake.on("plugin.list", Reply::Result(json!({"plugins": []})));
    fake.on("plugin.action.invoke", Reply::Result(json!({})));
    let run = inbox("--pane", (0, ""));
    InboxLink::default()
        .open(&fake.client(), &run, Some(3), None)
        .await;
    assert_eq!(plain(&fake), 1);
    assert!(run.calls.borrow().is_empty());
    // Python's `pane` truthiness: an empty pane is no deep link
    let fake = listed("0.10.5");
    InboxLink::default()
        .open(&fake.client(), &run, None, Some(""))
        .await;
    assert_eq!(plain(&fake), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn not_installed_is_looked_up_again_after_a_minute_and_found_is_kept() {
    // test_r6_l1_not_installed_is_rechecked
    let fake = FakeHerdr::start();
    fake.on("plugin.list", Reply::Result(json!({"plugins": []})));
    let link = InboxLink::default();
    let t = Instant::now();
    assert_eq!(link.info(&fake.client(), t).await, None);
    assert_eq!(
        link.info(&fake.client(), t + RECHECK - Duration::from_secs(1))
            .await,
        None
    );
    assert_eq!(fake.params_of("plugin.list").len(), 1); // cached
    link.info(&fake.client(), t + RECHECK).await;
    assert_eq!(fake.params_of("plugin.list").len(), 2); // looked up again
    let fake = listed("0.10.5");
    let found = link.info(&fake.client(), t + RECHECK * 2).await;
    assert!(found.is_some());
    link.info(&fake.client(), t + RECHECK * 10).await;
    assert_eq!(fake.params_of("plugin.list").len(), 1); // a found inbox is kept
}

#[test]
fn the_command_runs_the_inbox_as_itself_without_panoramas_context() {
    // test_060_deep_links_with_the_inbox_environment and test_r6_l2_panoramas_context_does_not_leak
    let env = [
        "HERDR_PANE_ID",
        "HERDR_TAB_ID",
        "HERDR_PLUGIN_CONTEXT_JSON",
        "HERDR_PLUGIN_STATE_DIR",
        "UV_PROJECT",
        "VIRTUAL_ENV",
        "HERDR_SOCKET_PATH",
        "PATH",
    ];
    let cmd = command(
        "/inbox",
        &["open", "--pane", "w1:p1"],
        env.iter().map(|s| (*s).to_owned()),
    );
    let std = cmd.as_std();
    assert_eq!(std.get_program(), "uv");
    let args: Vec<_> = std
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        [
            "run",
            "--quiet",
            "--frozen",
            "--no-dev",
            "--project",
            "/inbox",
            "inbox",
            "open",
            "--pane",
            "w1:p1"
        ]
    );
    let envs: std::collections::HashMap<String, Option<String>> = std
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    for gone in [
        "HERDR_PANE_ID",
        "HERDR_TAB_ID",
        "HERDR_PLUGIN_CONTEXT_JSON",
        "HERDR_PLUGIN_STATE_DIR",
        "UV_PROJECT",
        "VIRTUAL_ENV",
    ] {
        assert_eq!(envs.get(gone), Some(&None), "{gone} leaks");
    }
    assert_eq!(
        envs.get("HERDR_PLUGIN_ID"),
        Some(&Some(INBOX_ID.to_owned()))
    );
    assert_eq!(
        envs.get("HERDR_PLUGIN_ROOT"),
        Some(&Some("/inbox".to_owned()))
    );
    assert!(!envs.contains_key("HERDR_SOCKET_PATH") && !envs.contains_key("PATH")); // kept as they are
    assert_eq!(std.get_current_dir(), Some(std::path::Path::new("/inbox")));
}

#[test]
fn versions_and_failure_words_read_as_python_reads_them() {
    assert_eq!(version("0.6"), (0, 6, 0));
    assert_eq!(version("0.10.5-dev"), (0, 10, 5));
    assert_eq!(version("v1.2.3"), (0, 0, 0));
    assert_eq!(version("1-2"), (0, 0, 0)); // Python's regex needs the dot
    assert!(before_popup("Error: Unrecognized arguments: --pane"));
    assert!(!before_popup("Traceback: boom"));
}
