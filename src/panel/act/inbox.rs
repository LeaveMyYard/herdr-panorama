//! herdr-inbox, opened on a question's group or a pane's (Python `inbox_link.py`). The inbox
//! is optional: found through herdr (`plugin.list`), its deep link (`inbox open --question
//! Qn | --pane P`, herdr-inbox >= 0.6.0) is run from its own plugin root with its own
//! environment. Without the deep link, or when it certainly failed before any popup, the
//! plain `open` action runs: also when no inbox is listed (the caller only offers the inbox
//! while the core says it is there, `inbox_ok`).
//!
//! What it learns (the inbox's root and version, its `open --help`) is cached in an
//! [`InboxLink`] that clones cheaply and is never locked across a herdr call or a run of the
//! inbox's CLI, so the actor can hand a clone to each task.

mod detect;
mod run;

pub use detect::{DEEP_LINK, before_popup, version};
pub use run::{Ran, Runner, UvRunner, command};

use herdr_agent_kit::herdr_api::Herdr;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The inbox's plugin id and its plain open action.
pub const INBOX_ID: &str = "leavemyyard.herdr-inbox";
pub const OPEN_ACTION: &str = "leavemyyard.herdr-inbox.open";
/// "Not installed" is looked up again after this; a found inbox is kept.
pub const RECHECK: Duration = Duration::from_secs(60);

/// The installed inbox: where it runs from, and its version.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InboxInfo {
    pub root: String,
    pub version: String,
}

#[derive(Debug, Default)]
struct Cache {
    /// The inbox as last looked up, and when.
    info: Option<(Option<InboxInfo>, Instant)>,
    /// Each inbox's `open --help` (`None`: it failed).
    help: HashMap<InboxInfo, Option<String>>,
}

/// The inbox link, with what it learnt (Python's `_cache`). Clones share it.
#[derive(Clone, Debug, Default)]
pub struct InboxLink {
    cache: Arc<Mutex<Cache>>,
}

impl InboxLink {
    fn cache(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn info(&self, h: &Herdr, now: Instant) -> Option<InboxInfo> {
        let cached = self.cache().info.clone();
        if let Some((info, at)) = cached
            && (info.is_some() || now.duration_since(at) < RECHECK)
        {
            return info;
        }
        let found = h.plugins().await.ok().and_then(|ps| {
            ps.into_iter()
                .find(|p| p.plugin_id.as_deref() == Some(INBOX_ID) && p.enabled)
                .map(|p| InboxInfo {
                    root: p.plugin_root.unwrap_or_default(),
                    version: p.version.unwrap_or_default(),
                })
        });
        let found = found.filter(|i| !i.root.is_empty());
        self.cache().info = Some((found.clone(), now));
        found
    }

    /// `inbox open --help` of this inbox, read once; `None` if it failed.
    async fn help(&self, info: &InboxInfo, run: &impl Runner) -> Option<String> {
        if let Some(known) = self.cache().help.get(info).cloned() {
            return known;
        }
        let ran = run.inbox(&info.root, &["open", "--help"]).await;
        let help = (ran.code == 0).then_some(ran.out);
        self.cache().help.insert(info.clone(), help.clone());
        help
    }

    /// Opens the inbox on `question`'s group, else on `pane`'s, else plain. Why it failed.
    pub async fn open(
        &self,
        h: &Herdr,
        run: &impl Runner,
        question: Option<i64>,
        pane: Option<&str>,
    ) -> Option<String> {
        let pane = pane.filter(|p| !p.is_empty()); // Python's `pane` truthiness
        let wanted = question.is_some() || pane.is_some();
        if wanted && let Some(info) = self.info(h, Instant::now()).await {
            let help = self.help(&info, run).await.unwrap_or_default();
            if version(&info.version) >= DEEP_LINK || help.contains("--pane") {
                let mut args = match question {
                    Some(q) => vec!["open".to_owned(), "--question".into(), format!("Q{q}")],
                    None => vec![
                        "open".to_owned(),
                        "--pane".into(),
                        pane.unwrap_or_default().to_owned(),
                    ],
                };
                if help.contains("--no-hold") {
                    args.push("--no-hold".into());
                }
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let ran = run.inbox(&info.root, &refs).await;
                if ran.code == 0 {
                    return None;
                }
                if !before_popup(&ran.out) {
                    return Some(format!(
                        "inbox open failed (exit {}); no second popup",
                        ran.code
                    ));
                }
            }
        }
        h.invoke_action(OPEN_ACTION)
            .await
            .err()
            .map(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests;
