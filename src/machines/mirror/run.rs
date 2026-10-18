//! One rsync run, bounded (Python `machines._run`): in its own process group, so a timeout
//! kills rsync and its ssh together.

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The longest error text kept from rsync's stderr, in characters.
const ERROR_CHARS: usize = 300;
/// How often the run is checked for its end.
const POLL: Duration = Duration::from_millis(10);

/// The text of a timeout.
pub const TIMED_OUT: &str = "rsync timed out";

/// rsync's exit 24: some source files vanished while it ran (Claude Code rewrote them).
const VANISHED: i32 = 24;

/// Runs `argv`; `None` when it succeeded (exit 0 or 24), else the error: its stderr (trimmed,
/// at most 300 characters), `rsync exit <n>`, [`TIMED_OUT`], or why it could not start.
pub fn run(argv: &[String], timeout: Duration) -> Option<String> {
    let (bin, args) = argv.split_first()?;
    let spawned = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return Some(format!("rsync: {e}")),
    };
    let stderr = drain(&mut child);
    let deadline = Instant::now() + timeout;
    let code = loop {
        match child.try_wait() {
            // Python's returncode: the exit code, or minus the signal that ended it
            Ok(Some(status)) => break status.code().or(status.signal().map(|s| -s)),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL),
            _ => {
                kill_group(&child);
                let _ = child.wait();
                return Some(TIMED_OUT.to_owned());
            }
        }
    };
    if matches!(code, Some(0 | VANISHED)) {
        return None;
    }
    // the pipe closes when rsync and its ssh are gone; never wait past the deadline for it
    let err = stderr
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_default();
    let err = String::from_utf8_lossy(&err);
    let text = match err.trim() {
        "" => format!("rsync exit {}", code.unwrap_or_default()),
        t => t.to_owned(),
    };
    Some(text.chars().take(ERROR_CHARS).collect())
}

/// stderr, read to its end on a thread.
fn drain(child: &mut Child) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    let pipe = child.stderr.take();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// SIGKILL to the child's whole process group (its group id is its pid).
fn kill_group(child: &Child) {
    let group = i32::try_from(child.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = group {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

#[cfg(test)]
mod tests;
