//! The sessions the collector follows (Python `Collector.session`, `live_session`,
//! `continuations`): one kit [`Scanner`] per Claude Code session, each update reading only
//! what was appended. A conversation that moved to a new session id is followed through its
//! `continued-in` records, at most [`CONTINUE_HOPS`] hops.

use super::paths::hooks_path;
use herdr_agent_kit::agent_session::claude::Scanner;
use herdr_agent_kit::agent_session::claude::io::ProjectsRoot;
use herdr_agent_kit::agent_session::{SessionId, Ts};
use std::collections::HashMap;
use std::path::PathBuf;

/// Session continuations followed at most.
pub const CONTINUE_HOPS: usize = 8;
/// Records older than the done window by this much are not read on a first update.
const SINCE_SLACK: f64 = 600.0;

/// The scanners, by session id as herdr or a transcript named it.
pub struct Sessions {
    projects: ProjectsRoot,
    hooks_dir: PathBuf,
    scanners: HashMap<String, Scanner>,
}

impl Sessions {
    /// Sessions under `projects`, with hook records in `hooks_dir`.
    pub fn new(projects: PathBuf, hooks_dir: PathBuf) -> Self {
        Self {
            projects: ProjectsRoot::new(projects),
            hooks_dir,
            scanners: HashMap::new(),
        }
    }

    /// A session's scanner, if it was found.
    pub fn get(&self, session_id: &str) -> Option<&Scanner> {
        self.scanners.get(session_id)
    }

    /// The session's scanner, updated (records since `since`); `None` when no transcript is
    /// found. A scanner whose transcript is gone looks it up again.
    pub fn session(&mut self, session_id: &str, cwd: Option<&str>, since: f64) -> Option<&Scanner> {
        if !self
            .scanners
            .get(session_id)
            .is_some_and(|s| s.root().is_file())
        {
            let id = SessionId::parse(session_id)?;
            let path = self.projects.find(&id, cwd)?;
            let hooks = hooks_path(&self.hooks_dir, session_id);
            let scanner = Scanner::new(id, path, cwd.map(str::to_owned), Some(hooks));
            self.scanners.insert(session_id.to_owned(), scanner);
        }
        let s = self.scanners.get_mut(session_id)?;
        s.update(Some(Ts(since)));
        Some(s)
    }

    fn continued_in(&self, key: &str) -> Option<String> {
        let s = self.scanners.get(key)?;
        s.session()
            .conversation()
            .continued_in()
            .map(|s| s.to_string())
    }

    /// The session a conversation lives in now: every id on the way (oldest first), and the
    /// key of the last session found. A loop stops at the first repeat; a next session not
    /// written yet (or on another machine) ends the walk at the last one found.
    pub fn live_session(
        &mut self,
        session_id: &str,
        cwd: Option<&str>,
        since: f64,
    ) -> (Vec<String>, Option<String>) {
        let mut ids = vec![session_id.to_owned()];
        let mut key = self
            .session(session_id, cwd, since)
            .map(|_| session_id.to_owned());
        while let Some(k) = key.clone() {
            let Some(next) = self.continued_in(&k) else {
                break;
            };
            if ids.len() > CONTINUE_HOPS || ids.contains(&next) {
                break;
            }
            if self.session(&next, cwd, since).is_none() {
                break;
            }
            ids.push(next.clone());
            key = Some(next);
        }
        (ids, key)
    }

    /// The sessions these ones continued in, as far as the transcripts read so far tell,
    /// including a next one not found yet (a remote mirror then copies it), none of them in
    /// `session_ids`.
    pub fn continuations(&self, session_ids: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for sid in session_ids {
            let mut seen = vec![sid.clone()];
            let mut cur = sid.clone();
            while let Some(next) = self.continued_in(&cur) {
                if seen.contains(&next) || seen.len() > CONTINUE_HOPS {
                    break;
                }
                seen.push(next.clone());
                if !out.contains(&next) && !session_ids.contains(&next) {
                    out.push(next.clone());
                }
                cur = next;
            }
        }
        out
    }
}

/// The oldest record time a scanner reads on its first update.
pub fn since(now: f64, window: f64) -> f64 {
    now - window - SINCE_SLACK
}

#[cfg(test)]
mod tests;
