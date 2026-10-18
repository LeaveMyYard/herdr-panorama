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

fn herdr(version: &str) -> FakeHerdr {
    let fake = FakeHerdr::start();
    fake.on(
        "plugin.list",
        Reply::Result(json!({"plugins": [
        {"plugin_id": INBOX_ID, "version": version, "plugin_root": "/inbox", "enabled": true}]})),
    );
    fake.on("plugin.action.invoke", Reply::Result(json!({})));
    fake
}

#[tokio::test(flavor = "current_thread")]
async fn a_question_opens_its_group_through_the_deep_link() {
    let fake = herdr("0.10.5");
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
    assert!(fake.params_of("plugin.action.invoke").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn a_deep_link_that_failed_before_the_popup_falls_back_to_the_plain_one() {
    let fake = herdr("0.10.5");
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
    let fake = herdr("0.10.5");
    let run = inbox("--pane", (1, "boom"));
    let err = InboxLink::default()
        .open(&fake.client(), &run, None, Some("w1:p1"))
        .await;
    assert_eq!(
        err.as_deref(),
        Some("inbox open failed (exit 1); no second popup")
    );
    assert!(fake.params_of("plugin.action.invoke").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn an_old_inbox_or_none_opens_plain_or_nothing() {
    // older than 0.6.0 and its help has no --pane: the plain action
    let fake = herdr("0.5.9");
    let run = inbox("usage: open", (0, ""));
    InboxLink::default()
        .open(&fake.client(), &run, Some(1), None)
        .await;
    assert_eq!(fake.params_of("plugin.action.invoke").len(), 1);
    assert!(
        !run.calls
            .borrow()
            .iter()
            .any(|c| c.contains(&"--question".to_owned()))
    );
    assert_eq!(version("0.6"), (0, 6, 0));
    assert_eq!(version("v1.2.3"), (0, 0, 0));
}
