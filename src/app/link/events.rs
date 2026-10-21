//! herdr's focus events for the home view (Python `SwarmApp._watch_focus`, `focus_event`):
//! they feed the panels' focus log, tell the view which tab is shown and which pane has the
//! focus, and, once a burst settles (`FOLLOW_DEBOUNCE`), ask for one panels pass.

use super::done::Done;
use crate::panels::{Focus, FocusLog};
use herdr_agent_kit::herdr_api::{EventKind, Herdr, HerdrEvent};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// A tab switch sends tab_focused and pane_focused, prefix+1..9 can come in a row: act once
/// on where the operator ended up.
pub const FOLLOW_DEBOUNCE: Duration = Duration::from_millis(150);

/// Starts the watcher; it ends when the view is gone (its channel closed). `settled`: a burst
/// of focus events ended (the panels' pass).
pub fn spawn(
    h: Herdr,
    log: Arc<FocusLog>,
    done: mpsc::UnboundedSender<Done>,
    settled: impl Fn() + Send + 'static,
) {
    tokio::spawn(async move {
        let mut events = h.events(&[EventKind::TAB_FOCUSED, EventKind::PANE_FOCUSED]);
        let mut due: Option<tokio::time::Instant> = None;
        loop {
            let ev = tokio::select! {
                ev = events.next() => ev,
                () = sleep_until(due) => {
                    due = None;
                    settled(); // the operator may have entered a tab that has no panel yet
                    continue;
                }
            };
            let sent = match ev {
                HerdrEvent::Connected => {
                    log.set_live(true);
                    // where the keyboard is now: terminals report focus only on change
                    match h.snapshot().await {
                        Ok(s) => {
                            let tab = s
                                .focused_tab_id
                                .map(|t| Done::TabShown(t.as_str().to_owned()));
                            let pane = s
                                .focused_pane_id
                                .map(|p| Done::HerdrFocus(p.as_str().to_owned()));
                            [tab, pane]
                                .into_iter()
                                .flatten()
                                .all(|d| done.send(d).is_ok())
                        }
                        Err(_) => true,
                    }
                }
                HerdrEvent::Lost => {
                    log.set_live(false);
                    true
                }
                HerdrEvent::Event(e) => {
                    let (tab, pane) = (e.tab_id(), e.pane_id());
                    if matches!(e.kind.as_str(), "tab_focused" | "pane_focused") {
                        due = Some(tokio::time::Instant::now() + FOLLOW_DEBOUNCE);
                    }
                    let out = match e.kind.as_str() {
                        "tab_focused" => tab.map(|t| {
                            (
                                Focus::Tab(t.as_str().into()),
                                Done::TabShown(t.as_str().into()),
                            )
                        }),
                        "pane_focused" => pane.map(|p| {
                            (
                                Focus::Pane(p.as_str().into()),
                                Done::HerdrFocus(p.as_str().into()),
                            )
                        }),
                        _ => None,
                    };
                    match out {
                        Some((f, d)) => {
                            log.record(f);
                            done.send(d).is_ok()
                        }
                        None => true,
                    }
                }
                _ => true,
            };
            if !sent {
                return; // the view is gone
            }
        }
    });
}

async fn sleep_until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
