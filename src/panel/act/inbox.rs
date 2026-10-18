//! herdr-inbox, opened on a question's group or a pane's (Python `inbox_link.py`). The inbox
//! is optional: found through herdr (`plugin.list`), its deep link (`inbox open --question
//! Qn | --pane P`, herdr-inbox >= 0.6.0) is run from its own plugin root with its own
//! environment. Without it, or when the deep link certainly failed before any popup, the
//! plain `open` action runs; not installed, nothing does.

mod run;

pub use run::{Ran, Runner, UvRunner};

use herdr_agent_kit::herdr_api::Herdr;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The inbox's plugin id and its plain open action.
pub const INBOX_ID: &str = "leavemyyard.herdr-inbox";
pub const OPEN_ACTION: &str = "leavemyyard.herdr-inbox.open";
/// The first version with the deep link.
pub const DEEP_LINK: (u64, u64, u64) = (0, 6, 0);
/// "Not installed" is looked up again after this; a found inbox is kept.
pub const RECHECK: Duration = Duration::from_secs(60);

/// The installed inbox: where it runs from, and its version.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InboxInfo {
    pub root: String,
    pub version: String,
}

/// `major.minor[.patch]` at the start of `text` (0 for none).
fn version(text: &str) -> (u64, u64, u64) {
    let mut parts = text.split(|c: char| !c.is_ascii_digit());
    let mut next = || parts.next().and_then(|p| p.parse().ok());
    match (next(), next()) {
        (Some(a), Some(b)) => (a, b, next().unwrap_or(0)),
        _ => (0, 0, 0),
    }
}

/// Words in a failed deep link's output that mean it failed before any popup opened.
fn before_popup(out: &str) -> bool {
    let out = out.to_lowercase();
    [
        "could not open the inbox popup",
        "unrecognized arguments",
        "invalid choice",
        "unable to find lockfile",
        "no such file or directory",
        "failed to spawn",
    ]
    .iter()
    .any(|w| out.contains(w))
}

/// The inbox link, with what it learnt cached (Python's `_cache`).
#[derive(Debug, Default)]
pub struct InboxLink {
    info: Option<(Option<InboxInfo>, Instant)>,
    help: HashMap<InboxInfo, Option<String>>,
}

impl InboxLink {
    async fn info(&mut self, h: &Herdr) -> Option<InboxInfo> {
        let stale = matches!(&self.info, Some((None, at)) if at.elapsed() >= RECHECK);
        if self.info.is_none() || stale {
            let found = h.plugins().await.ok().and_then(|ps| {
                ps.into_iter()
                    .find(|p| p.plugin_id.as_deref() == Some(INBOX_ID) && p.enabled)
                    .map(|p| InboxInfo {
                        root: p.plugin_root.unwrap_or_default(),
                        version: p.version.unwrap_or_default(),
                    })
            });
            self.info = Some((found.filter(|i| !i.root.is_empty()), Instant::now()));
        }
        self.info.as_ref().and_then(|(i, _)| i.clone())
    }

    /// `inbox open --help` of this inbox, read once; `None` if it failed.
    async fn help(&mut self, info: &InboxInfo, run: &impl Runner) -> Option<String> {
        if !self.help.contains_key(info) {
            let ran = run.inbox(&info.root, &["open", "--help"]).await;
            self.help
                .insert(info.clone(), (ran.code == 0).then_some(ran.out));
        }
        self.help[info].clone()
    }

    /// Opens the inbox on `question`'s group, else on `pane`'s, else plain. Why it failed.
    pub async fn open(
        &mut self,
        h: &Herdr,
        run: &impl Runner,
        question: Option<i64>,
        pane: Option<&str>,
    ) -> Option<String> {
        let info = self.info(h).await;
        if let (Some(info), true) = (&info, question.is_some() || pane.is_some()) {
            let help = self.help(info, run).await.unwrap_or_default();
            let deep = version(&info.version) >= DEEP_LINK || help.contains("--pane");
            if deep {
                let mut args: Vec<String> = match (question, pane) {
                    (Some(q), _) => vec!["open".into(), "--question".into(), format!("Q{q}")],
                    (None, Some(p)) => vec!["open".into(), "--pane".into(), p.into()],
                    (None, None) => unreachable!("checked above"),
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
