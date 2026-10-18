//! The app's panic hook: a panic is written to its log (`tui.log` in panorama's state dir,
//! the last 16 KB kept), never onto the terminal while the view draws there. A panic that is
//! contained (the collect thread's, a guarded task's) only goes to the log; any other puts
//! the terminal back and then lets `next` (the default hook) speak.
//!
//! Containment needs unwinding: no profile may set `panic = "abort"` (a test checks).

use super::collect::THREAD;
use crate::ui::{clock, crash, guard};
use std::fs;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// The log, in panorama's state dir.
pub fn log_path(state_dir: &Path) -> PathBuf {
    state_dir.join("tui.log")
}

/// One writer at a time: panics on two threads must not lose each other's lines.
static LOG: Mutex<()> = Mutex::new(());

thread_local! {
    /// This thread is writing the log: a panic inside the write skips logging instead of
    /// waiting on its own lock.
    static WRITING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Appends `text` to the log, which keeps its last `crash::LOG_BYTES`. Never fails, never
/// waits on itself (a panic while writing is not logged).
pub fn log(path: &Path, text: &str) {
    if WRITING.with(|w| w.replace(true)) {
        return;
    }
    let _done = Reset;
    let _one = LOG.lock().unwrap_or_else(PoisonError::into_inner);
    let old = fs::read_to_string(path).unwrap_or_default();
    let all = format!("{old}{text}\n");
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, crash::tail(&all));
}

/// Clears this thread's [`WRITING`] when its write ends (on a panic too).
struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        WRITING.with(|w| w.set(false));
    }
}

/// Whether a panic here is contained: on the collect thread, or in a guarded task.
fn contained() -> bool {
    guard::in_task() || std::thread::current().name() == Some(THREAD)
}

/// The hook: `log` is the log's path; `restore` puts the terminal back.
pub fn hook(
    log_at: PathBuf,
    restore: fn(),
    next: Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync>,
) -> Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync> {
    Box::new(move |info| {
        log(&log_at, &format!("{} {info}", clock::stamp()));
        if contained() {
            return;
        }
        restore();
        next(info);
    })
}

#[cfg(test)]
mod tests;
