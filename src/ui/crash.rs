//! The crash marker (Python `panel.mark_crash`): a viewer that dies of an error leaves
//! `panel-crash-<pane>` in the state dir, so the core does not take its vanished pane for the
//! operator closing it, and `panel-crash-<pane>.log` with the error's last 16 KB as evidence.

use std::fs;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The log keeps this many bytes of the error's end.
pub const LOG_BYTES: usize = 16384;

/// The marker for `pane` in `state_dir`.
pub fn marker(state_dir: &Path, pane: &str) -> PathBuf {
    state_dir.join(format!("panel-crash-{pane}"))
}

/// The last `LOG_BYTES` of `text`, cut at a character boundary.
fn tail(text: &str) -> &str {
    let mut start = text.len().saturating_sub(LOG_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// Leaves the marker (and the log, with `stamp` as its first line). Never fails: it runs
/// while a crash is already on its way out.
pub fn mark(state_dir: &Path, pane: Option<&str>, error: &str, stamp: &str) {
    let Some(pane) = pane.filter(|p| !p.is_empty()) else {
        return;
    };
    let path = marker(state_dir, pane);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if fs::create_dir_all(state_dir).is_err() || fs::write(&path, now.to_string()).is_err() {
        return;
    }
    let log = state_dir.join(format!("panel-crash-{pane}.log"));
    let _ = fs::write(log, format!("{stamp}\n{}", tail(error)));
}

/// What a panic does to the panel (installed as the panic hook): the terminal is put back
/// (`restore`), the marker and log are written, then `next` (the default hook) speaks. A
/// panic in a task of the herdr side is left to that task: it reports its own failure and
/// the panel goes on (`actor::guard`).
pub fn hook(
    state_dir: PathBuf,
    pane: Option<String>,
    restore: fn(),
    next: Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync>,
) -> Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync> {
    Box::new(move |info| {
        if super::guard::in_task() {
            return;
        }
        restore();
        mark(
            &state_dir,
            pane.as_deref(),
            // the panic and where it happened, as Python's log keeps the traceback
            &format!("{info}\n{}", std::backtrace::Backtrace::force_capture()),
            &super::clock::stamp(),
        );
        next(info);
    })
}

#[cfg(test)]
mod tests;
