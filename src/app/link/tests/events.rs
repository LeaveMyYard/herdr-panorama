//! herdr's focus events: the focus log, the tab shown, the focused pane, and one panels pass
//! once a burst settles.

use crate::app::link::Done;
use crate::app::link::events::{FOLLOW_DEBOUNCE, spawn};
use crate::panels::{Focus, FocusLog};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

async fn next(rx: &mut mpsc::UnboundedReceiver<Done>) -> Done {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("a report")
        .expect("open")
}

#[tokio::test(flavor = "current_thread")]
async fn focus_events_feed_the_log_the_view_and_one_pass_per_burst() {
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":
        {"focused_tab_id": "w1:t1", "focused_pane_id": "w1:p1", "panes": []}})),
    );
    let (log, (tx, mut rx)) = (Arc::new(FocusLog::default()), mpsc::unbounded_channel());
    let passes = Arc::new(AtomicUsize::new(0));
    let p = passes.clone();
    spawn(fake.client(), log.clone(), tx, move || {
        p.fetch_add(1, Ordering::SeqCst);
    });
    while fake.subscribers() == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // once the stream is live: where the keyboard is now
    assert!(matches!(next(&mut rx).await, Done::TabShown(t) if t == "w1:t1"));
    assert!(matches!(next(&mut rx).await, Done::HerdrFocus(p) if p == "w1:p1"));
    assert!(log.live());
    for (tab, pane) in [("w1:t2", "w1:p2"), ("w1:t3", "w1:p3"), ("w1:t4", "w1:p4")] {
        fake.emit(json!({"type": "tab_focused", "tab_id": tab, "workspace_id": "w1"}));
        fake.emit(json!({"type": "pane_focused", "pane_id": pane}));
    }
    for want in ["w1:t2", "w1:p2", "w1:t3", "w1:p3", "w1:t4", "w1:p4"] {
        match next(&mut rx).await {
            Done::TabShown(t) | Done::HerdrFocus(t) => assert_eq!(t, want),
            other => panic!("{other:?}"),
        }
    }
    tokio::time::sleep(FOLLOW_DEBOUNCE * 3).await;
    assert_eq!(
        passes.load(Ordering::SeqCst),
        1,
        "a burst: one pass, where the operator ended up"
    );
    assert!(
        log.all().contains(&Focus::Pane("w1:p4".into()))
            && log.all().contains(&Focus::Tab("w1:t2".into()))
    );
    fake.emit(json!({"type": "pane_agent_status_changed", "pane_id": "w1:p2"}));
    tokio::time::sleep(FOLLOW_DEBOUNCE * 3).await;
    assert_eq!(
        passes.load(Ordering::SeqCst),
        1,
        "other events start no pass"
    );
    fake.drop_subscribers();
    for _ in 0..200 {
        if !log.live() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        !log.live(),
        "a lost stream: the restore check decides nothing"
    );
}
