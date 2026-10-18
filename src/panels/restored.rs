//! After herdr itself stopped unexpectedly (Python `Manager._restored`, `_judge`,
//! `_remember`): herdr's session restore brings every pane back with its pane id and label,
//! but as a plain login shell. A recorded panel of ours is closed then only when every check
//! below says it is still our unused panel (docs/DESIGN.md "As built"); any other answer,
//! "herdr cannot say yet" excepted, gives the pane up for good, and so does not deciding
//! within `RESTORE_PASSES` passes. Leaving a shell is safe; closing a pane the operator used
//! is not.

use super::Manager;
use super::focus_log::Focus;
use super::ops::{Ops, Shell};
use super::record::Recorded;
use super::snap::{Snap, SnapPane};
use super::survey::Survey;
use std::collections::BTreeSet;

/// What the restore check decided for one pane.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
    /// Closed now.
    Close,
    /// herdr could not say yet: look again next pass.
    Again,
    /// Left alone for good: why.
    Leave(String),
}

impl<O: Ops> Manager<O> {
    /// Closes the recorded panels herdr restored as plain shells; the snapshot without them.
    pub(super) async fn restored(&mut self, mut snap: Snap) -> Snap {
        let mut changed = false;
        let mut candidates = vec![];
        for (pane, rec) in self.saved.panes.clone() {
            let Some(p) = snap.pane(&pane).cloned() else {
                // the pane is gone (or a fresh server numbers panes anew)
                self.saved.panes.remove(&pane);
                changed = true;
                continue;
            };
            if !rec.terminal.is_empty() && p.terminal_id.as_deref().is_some_and(|now| !now.is_empty() && now != rec.terminal) {
                candidates.push(p); // a restore, or a live handoff (the panel still runs)
            }
        }
        if !candidates.is_empty() && self.restore_seen.is_none() {
            self.restore_seen = Some(self.passes);
        }
        let since = self.restore_seen.map_or(0, |s| self.passes - s);
        let mut gone = BTreeSet::new();
        for p in candidates {
            let verdict = if since < super::RESTORE_PASSES { self.judge(&p, &snap).await } else { Verdict::Leave("late".into()) };
            if verdict == Verdict::Again {
                continue;
            }
            self.saved.panes.remove(&p.pane_id);
            changed = true;
            match verdict {
                Verdict::Close => {
                    gone.insert(p.pane_id.clone());
                    self.created.remove(&p.pane_id);
                    self.connected_once.remove(&p.pane_id);
                }
                Verdict::Leave(why) => self.note(format!("{} was a panel before herdr restarted; left alone ({why})", p.pane_id)),
                Verdict::Again => {}
            }
        }
        if changed {
            self.save();
        }
        snap.panes.retain(|p| !gone.contains(&p.pane_id));
        snap
    }

    /// One recorded pane with a new terminal. A panel that still runs after a live handoff is
    /// "in use" here, and `remember` records it again under its new terminal.
    async fn judge(&mut self, p: &SnapPane, snap: &Snap) -> Verdict {
        let leave = |why: &str| Verdict::Leave(why.into());
        if p.label.as_deref() != Some(crate::panels::PANEL_LABEL) {
            return leave("relabelled");
        }
        if p.agent.as_deref().is_some_and(|a| !a.is_empty()) {
            return leave("an agent");
        }
        if p.focused || snap.focused_pane.as_deref() == Some(&p.pane_id) || snap.tab_remembers(&p.pane_id) {
            return leave("focused");
        }
        match self.focused_ever(&p.pane_id) {
            None => return Verdict::Again, // no live focus stream: never decide without it
            Some(true) => return leave("focused"),
            Some(false) => {}
        }
        let shell = match self.ops.pane_shell(&p.pane_id).await {
            Shell::Unknown => return Verdict::Again,
            Shell::Busy => return leave("in use"), // never looked at again
            Shell::Plain(pid) => pid,
        };
        let (age, children) = self.ops.shell_state(shell).await;
        if children != Some(false) {
            return leave("in use"); // a background job, or unknown
        }
        if age.is_none_or(|a| a > super::RESTORED_SECONDS) {
            return leave("not a fresh restore");
        }
        // the last look right before the close: herdr has no conditional close, so a command
        // typed in the few ms between this look and the close would still go with the pane
        if self.ops.pane_shell(&p.pane_id).await != Shell::Plain(shell) {
            return leave("in use");
        }
        if self.focused_ever(&p.pane_id) != Some(false) {
            return leave("focused"); // a click during the looks above
        }
        if let Err(e) = self.ops.close_pane(&p.pane_id).await {
            return Verdict::Leave(format!("could not close: {e}"));
        }
        self.note(format!("closed {}: a panel herdr restored as a plain shell; a new one is made", p.pane_id));
        Verdict::Close
    }

    /// Whether this core's focus stream has seen the pane focused, ever; `None` when there is
    /// no live stream to ask.
    fn focused_ever(&self, pane: &str) -> Option<bool> {
        let log = self.focus_log.as_ref().filter(|l| l.live())?;
        Some(log.all().iter().any(|e| matches!(e, Focus::Pane(p) if p == pane)))
    }

    /// Records our running panels (pane id, tab, herdr's terminal id) for after a herdr
    /// restart: only panes this core opened or adopted, while herdr shows them running.
    pub(super) fn remember(&mut self, snap: &Snap, sv: &Survey) {
        let running: BTreeSet<&String> = sv.panels.values().flatten().collect();
        let mut record = self.saved.panes.clone();
        for (pane, tab) in &self.created {
            if let Some(term) = snap.pane(pane).and_then(|p| p.terminal_id.clone()).filter(|_| running.contains(pane)) {
                record.insert(pane.clone(), Recorded { tab: tab.clone(), terminal: term });
            }
        }
        if record != self.saved.panes {
            self.saved.panes = record;
            self.save();
        }
    }
}
