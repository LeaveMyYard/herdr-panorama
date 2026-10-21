//! Whether this rsync knows `--append-verify` (Python `machines.append_supported`): rsync 3.x
//! does, macOS's openrsync does not. Asked once per process, on first use (on a watcher's
//! thread, never the view's), with `rsync --help` killed at its deadline.

use super::bounded::{Ended, bounded};
use std::sync::OnceLock;
use std::time::Duration;

/// How long `rsync --help` may take.
pub const HELP_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether `<bin> --help` names `--append-verify` (on stdout or stderr) within `timeout`.
pub fn knows_append_verify(bin: &str, timeout: Duration) -> bool {
    match bounded(
        &[bin.to_owned(), "--help".to_owned()],
        timeout,
        true,
        &|| false,
    ) {
        Ended::Exited { stdout, stderr, .. } => {
            String::from_utf8_lossy(&[stdout, stderr].concat()).contains("--append-verify")
        }
        _ => false,
    }
}

/// [`knows_append_verify`] of `rsync`, asked once per process.
pub fn append_supported() -> bool {
    static KNOWN: OnceLock<bool> = OnceLock::new();
    *KNOWN.get_or_init(|| knows_append_verify("rsync", HELP_TIMEOUT))
}

#[cfg(test)]
mod tests;
