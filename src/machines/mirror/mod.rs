//! Mirroring what the scanner reads of some sessions with rsync (Python `machines.mirror`):
//! two passes into the destination, the `.jsonl` files append-only (`--append-verify`: only
//! new bytes travel), then the `.json` files normally (they are rewritten in place, maybe
//! shorter or at the same size). No file over [`MAX_FILE`].

pub mod rules;
pub mod run;

pub use rules::{Kind, rules};
pub use run::{TIMED_OUT, run};

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

/// rsync `--max-size`: no single mirrored file is larger.
pub const MAX_FILE: &str = "64M";
/// One rsync pass's timeout.
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(30);
/// How long `rsync --help` may take.
const HELP_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs one rsync argv within a timeout: `None` on success, else the error ([`run`]).
pub type Runner = dyn Fn(&[String], Duration) -> Option<String> + Send + Sync;

/// How the passes run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rsync {
    /// `--max-size`.
    pub max_file: String,
    /// Whether this rsync knows `--append-verify`.
    pub append: bool,
    /// The remote shell (`-e`), none for a local source.
    pub ssh: Option<Vec<String>>,
    /// Each pass's timeout.
    pub timeout: Duration,
}

impl Rsync {
    /// The defaults, with `ssh` as the remote shell and this rsync's own `--append-verify`.
    pub fn new(ssh: Option<Vec<String>>) -> Self {
        Self {
            max_file: MAX_FILE.to_owned(),
            append: append_supported(),
            ssh,
            timeout: SYNC_TIMEOUT,
        }
    }

    /// The two passes' argvs: `.jsonl` (append-only when rsync can), then `.json`.
    pub fn passes(&self, source: &str, dest: &Path, sessions: &[String]) -> [Vec<String>; 2] {
        let pass = |kind: Kind| {
            let mut argv: Vec<String> = ["rsync", "-rt", "--prune-empty-dirs"]
                .map(str::to_owned)
                .to_vec();
            argv.push(format!("--max-size={}", self.max_file));
            if let Some(ssh) = &self.ssh {
                argv.extend(["-e".to_owned(), ssh.join(" ")]);
            }
            if kind == Kind::Jsonl && self.append {
                argv.push("--append-verify".to_owned());
            }
            argv.extend(rules(sessions, kind));
            argv.extend([source.to_owned(), format!("{}/", dest.display())]);
            argv
        };
        [pass(Kind::Jsonl), pass(Kind::Json)]
    }

    /// Both passes, through `runner`; the first error stops it.
    pub fn mirror(
        &self,
        source: &str,
        dest: &Path,
        sessions: &[String],
        runner: impl Fn(&[String], Duration) -> Option<String>,
    ) -> Option<String> {
        self.passes(source, dest, sessions)
            .iter()
            .find_map(|argv| runner(argv, self.timeout))
    }
}

/// Whether this rsync knows `--append-verify` (rsync 3.x does; macOS's openrsync does not).
/// Without it the `.jsonl` pass uses rsync's normal delta transfer, which also sends little
/// more than the appended bytes. Asked once per process.
pub fn append_supported() -> bool {
    static KNOWN: OnceLock<bool> = OnceLock::new();
    *KNOWN.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let out = Command::new("rsync")
                .arg("--help")
                .stdin(Stdio::null())
                .output();
            let _ = tx.send(out.map(|o| [o.stdout, o.stderr].concat()));
        });
        match rx.recv_timeout(HELP_TIMEOUT) {
            Ok(Ok(text)) => String::from_utf8_lossy(&text).contains("--append-verify"),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests;
