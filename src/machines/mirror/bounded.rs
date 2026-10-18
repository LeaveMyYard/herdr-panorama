//! A child process with a deadline: started in its own process group, and on timeout the whole
//! group is killed (rsync and the ssh it started together). Its output pipes are read on
//! threads, and never waited for past the deadline.

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the child is checked for its end.
const POLL: Duration = Duration::from_millis(10);
/// How often a spawn of a busy binary is retried, and how long apart.
const SPAWN_TRIES: u32 = 5;
const SPAWN_RETRY: Duration = Duration::from_millis(20);

/// How a bounded run ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    /// It exited: Python's returncode (the exit code, or minus the signal), and what it wrote
    /// on stdout (when asked for) and stderr.
    Exited {
        code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// The deadline passed; its group was killed.
    TimedOut,
    /// It could not start.
    NotStarted(String),
}

/// Runs `argv` (no stdin) for at most `timeout`; stdout is kept when `stdout` is true.
pub fn bounded(argv: &[String], timeout: Duration, stdout: bool) -> Ended {
    let Some((bin, args)) = argv.split_first() else {
        return Ended::NotStarted("no command".into());
    };
    let mut command = Command::new(bin);
    let out_pipe = if stdout {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(out_pipe)
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = match spawn(&mut command) {
        Ok(child) => child,
        Err(e) => return Ended::NotStarted(e.to_string()),
    };
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().or(status.signal().map(|s| -s)),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL),
            _ => {
                kill_group(&child);
                let _ = child.wait();
                return Ended::TimedOut;
            }
        }
    };
    let collect = |rx: mpsc::Receiver<Vec<u8>>| {
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
    };
    Ended::Exited {
        code,
        stdout: collect(out),
        stderr: collect(err),
    }
}

/// Spawns, retrying a few times while the binary is busy being written (`ETXTBSY`: an
/// update in progress, or a file another thread's fork briefly holds open). Nothing ran, so a
/// retry is safe (the kit's CLI runner does the same).
fn spawn(command: &mut Command) -> std::io::Result<Child> {
    let mut tries = 0;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < SPAWN_TRIES => {
                tries += 1;
                std::thread::sleep(SPAWN_RETRY);
            }
            spawned => return spawned,
        }
    }
}

/// A pipe, read to its end on a thread.
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
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
