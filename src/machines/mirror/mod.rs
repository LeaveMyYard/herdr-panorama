//! Mirroring what the scanner reads of some sessions with rsync (Python `machines.mirror`):
//! two passes into the destination, the `.jsonl` files append-only (`--append-verify`: only
//! new bytes travel), then the `.json` files normally (they are rewritten in place, maybe
//! shorter or at the same size). No file over [`MAX_FILE`].

pub mod bounded;
pub mod probe;
pub mod rules;
pub mod run;

pub use probe::append_supported;
pub use rules::{Kind, rules};
pub use run::{STOPPED, TIMED_OUT, run};

use std::path::Path;
use std::time::Duration;

/// rsync `--max-size`: no single mirrored file is larger.
pub const MAX_FILE: &str = "64M";
/// One rsync pass's timeout.
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs one rsync argv within a timeout: `None` on success, else the error ([`run`]).
pub type Runner = dyn Fn(&[String], Duration, &dyn Fn() -> bool) -> Option<String> + Send + Sync;

/// How the passes run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rsync {
    /// `--max-size`.
    pub max_file: String,
    /// Whether this rsync knows `--append-verify`; `None`: ask it on first use
    /// ([`append_supported`], as Python asks inside `mirror`).
    pub append: Option<bool>,
    /// The remote shell (`-e`), none for a local source.
    pub ssh: Option<Vec<String>>,
    /// Each pass's timeout.
    pub timeout: Duration,
}

impl Rsync {
    /// The defaults, with `ssh` as the remote shell; whether rsync knows `--append-verify` is
    /// asked on first use, so building one never runs rsync.
    pub fn new(ssh: Option<Vec<String>>) -> Self {
        Self {
            max_file: MAX_FILE.to_owned(),
            append: None,
            ssh,
            timeout: SYNC_TIMEOUT,
        }
    }

    /// The two passes' argvs: `.jsonl` (append-only when rsync can), then `.json`.
    pub fn passes(&self, source: &str, dest: &Path, sessions: &[String]) -> [Vec<String>; 2] {
        let append = self.append.unwrap_or_else(append_supported);
        let pass = |kind: Kind| {
            let mut argv: Vec<String> = ["rsync", "-rt", "--prune-empty-dirs"]
                .map(str::to_owned)
                .to_vec();
            argv.push(format!("--max-size={}", self.max_file));
            if let Some(ssh) = &self.ssh {
                argv.extend(["-e".to_owned(), ssh.join(" ")]);
            }
            if kind == Kind::Jsonl && append {
                argv.push("--append-verify".to_owned());
            }
            argv.extend(rules(sessions, kind));
            argv.extend([source.to_owned(), format!("{}/", dest.display())]);
            argv
        };
        [pass(Kind::Jsonl), pass(Kind::Json)]
    }

    /// Both passes, through `runner`; the first error stops it, and so does `cancelled()`
    /// (checked before each pass and, by the runner, while one runs).
    pub fn mirror(
        &self,
        source: &str,
        dest: &Path,
        sessions: &[String],
        runner: impl Fn(&[String], Duration, &dyn Fn() -> bool) -> Option<String>,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<String> {
        self.passes(source, dest, sessions).iter().find_map(|argv| {
            if cancelled() {
                return Some(STOPPED.to_owned());
            }
            runner(argv, self.timeout, cancelled)
        })
    }
}

#[cfg(test)]
mod tests;
