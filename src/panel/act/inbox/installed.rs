//! Constrained app navigation: existing installed CLI only, with no action fallback.
use super::*;

#[derive(Clone, Debug)]
pub struct InstalledInbox {
    root: String,
    pub questions: bool,
    no_hold: bool,
}
impl InboxLink {
    /// Only successful installed `open --help` proves this entrypoint available.
    pub async fn installed(&self, h: &Herdr, run: &impl Runner) -> Option<InstalledInbox> {
        let info = self.info(h, Instant::now()).await?;
        let help = self.help(&info, run).await?;
        Some(InstalledInbox {
            root: info.root,
            questions: help.contains("--question"),
            no_hold: help.contains("--no-hold"),
        })
    }
}
impl InstalledInbox {
    /// Returns only the installed CLI result. No retry or registered-action fallback.
    pub async fn open(&self, run: &impl Runner, question: Option<i64>) -> Result<(), String> {
        if question.is_some_and(|q| !(1..=9_007_199_254_740_991).contains(&q)) {
            return Err("invalid question id".into());
        }
        if question.is_some() && !self.questions {
            return Err("installed inbox has no question navigation".into());
        }
        let mut args = vec!["open".to_owned()];
        if let Some(q) = question {
            args.extend(["--question".into(), format!("Q{q}")]);
        }
        if self.no_hold {
            args.push("--no-hold".into());
        }
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        let result = run.inbox(&self.root, &refs).await;
        if result.code == 0 {
            Ok(())
        } else {
            Err(format!(
                "inbox navigation failed (exit {}); no retry",
                result.code
            ))
        }
    }
}
