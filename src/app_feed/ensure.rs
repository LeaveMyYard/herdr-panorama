//! Startup is serialized with installation; only a reserved lifetime FD reaches the child.
use super::{
    fs::{self, Home, Known, Result},
    journal::Journal,
    receipt::{BINARY, Ownership, Receipt},
    start::{LEDGER, Ledger, MAX_FAILURES},
};
use std::{
    fs::File,
    io::{ErrorKind, Read},
    os::{
        fd::AsFd,
        unix::{fs::MetadataExt, process::CommandExt},
    },
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// Generation-specific acknowledgement window; generous because startup under
/// heavy load can exceed seconds while still being a healthy writer.
pub const ACK: Duration = Duration::from_secs(5);
/// Covers a concurrent ensure's acknowledgement window plus its stop wait.
pub const LIFECYCLE_WAIT: Duration = Duration::from_secs(10);
const STOP_WAIT: Duration = Duration::from_secs(2);

enum Ack {
    Ready,
    /// Provenance failure: the child answered with another generation's bytes.
    Foreign,
    /// The child is gone before acknowledging; its lock may still be held by helpers.
    Exited(&'static str),
    /// No acknowledgement yet (or unreadable); the child may still be starting.
    Pending(String),
}

fn ready(child: &mut Child, id: &str) -> Ack {
    let Some(mut stdout) = child.stdout.take() else {
        return Ack::Pending("writer acknowledgement unavailable".into());
    };
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    if let Err(e) = fcntl_getfl(&stdout).and_then(|f| fcntl_setfl(&stdout, f | OFlags::NONBLOCK)) {
        return Ack::Pending(e.to_string());
    }
    let deadline = Instant::now() + ACK;
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0_u8; 256];
        match stdout.read(&mut chunk) {
            Ok(0) => return Ack::Exited("writer exited before acknowledgement"),
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Ack::Pending(e.to_string()),
        }
        let expected = format!("READY {id}\n");
        if !expected.as_bytes().starts_with(&bytes) {
            return Ack::Foreign;
        }
        if bytes == expected.as_bytes() {
            return Ack::Ready;
        }
        if Instant::now() >= deadline {
            return Ack::Pending("writer acknowledgement timed out".into());
        }
        match child.try_wait() {
            Ok(Some(_)) => return Ack::Exited("writer exited during startup"),
            Ok(None) => {}
            Err(e) => return Ack::Pending(e.to_string()),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The child registers before acknowledging, so a late acknowledgement from
/// this exact child is the same busy-writer proof a concurrent ensure accepts.
fn registered(home: &Home, r: &Receipt, pid: u32) -> bool {
    Ownership::read(home, r)
        .ok()
        .flatten()
        .is_some_and(|(owner, _)| owner.writer_pid == pid && owner.live(home))
}

/// Every spawn is counted before it happens, while the reserved lifetime lock
/// proves no writer runs (writers refuse a visible `.start.tmp`). Only a proven
/// start removes the count, so an attempt that is never proven, including one
/// left starting that later exits unregistered, stays inside the bound.
struct Attempt {
    failures: u32,
    /// Backoff already recorded for this attempt; `None` for the final attempt.
    wait: Option<u64>,
    /// The ledger file as this attempt left it, removed after a proven start.
    known: Option<Known>,
}
impl Attempt {
    fn count(home: &Home, r: &Receipt, ledger: Option<(Ledger, Known)>) -> Result<Self> {
        let failures = ledger.as_ref().map_or(0, |(l, _)| l.failures) + 1;
        if failures >= MAX_FAILURES {
            let known = ledger.map(|(_, k)| k);
            return Ok(Self {
                failures,
                wait: None,
                known,
            });
        }
        let wait = Ledger::record(home, r, failures, ledger.as_ref().map(|(_, k)| k))
            .map_err(|e| format!("start ledger unavailable ({e}); writer not started"))?;
        let known = Ledger::read(home, r)?.map(|(_, k)| k);
        Ok(Self {
            failures,
            wait: Some(wait),
            known,
        })
    }
    /// A transient failure: the attempt stays counted; the final one disables.
    fn failed(&self, home: &Home, r: &Receipt, error: &str) -> String {
        let (failures, max) = (self.failures, MAX_FAILURES);
        match self.wait {
            Some(wait) => {
                format!("{error}; transient start failure {failures}/{max}; retry after {wait} ms")
            }
            None => {
                let disabled = r.disable(home);
                format!(
                    "{error}; {failures} consecutive transient start failures; generation disabled: {disabled:?}"
                )
            }
        }
    }
    /// A proven start (or a permanent disable, which supersedes the count) ends the run.
    fn clear(&self, home: &Home, r: &Receipt) {
        if let Some(known) = &self.known {
            clear(home, r, known);
        }
    }
}

/// Advisory removal: a ledger whose bytes changed is left for review, with a warning.
fn clear(home: &Home, r: &Receipt, known: &Known) {
    if let Err(e) = home.remove(&r.path(LEDGER), known) {
        eprintln!("panorama app-feed: start ledger not reset: {e}");
    }
}

pub fn ensure(home: Home, expected: Option<&str>) -> Result<()> {
    fs::lock(&home.fd, LIFECYCLE_WAIT)?;
    let r = Receipt::read(&home)?;
    r.require_generation(expected)?;
    r.check_artifact(&home)?;
    r.enabled(&home)?;
    let lifetime = r.lock_file(&home)?;
    match lifetime.try_lock() {
        Ok(()) => {
            r.clear_staging(&home)?;
            r.check_names(&home)?;
            Journal::validate(&home, &r)?;
        }
        Err(std::fs::TryLockError::WouldBlock) => {
            r.check_busy_names(&home)?;
            let (owner, _) =
                Ownership::read(&home, &r)?.ok_or("busy writer has no ownership registration")?;
            if !owner.live(&home) {
                return Err("busy writer ownership cannot be proven".into());
            }
            // A proven running writer is a proven start: the failure run is over.
            if let Ok(Some((_, known))) = Ledger::read(&home, &r) {
                clear(&home, &r, &known);
            }
            return Ok(());
        }
        Err(e) => return Err(format!("writer lock failed: {e}")),
    }
    let ledger = Ledger::read(&home, &r)?;
    if let Some((l, _)) = &ledger
        && let Some(wait) = l.deferred_ms()
    {
        return Err(format!(
            "writer start deferred {wait} ms after {} transient start failures",
            l.failures
        ));
    }
    let attempt = Attempt::count(&home, &r, ledger)?;
    // A spawn error drops the reservation with the command; no writer can run.
    let spawned = Command::new(home.path.join(BINARY))
        .args(["app-feed", "--watch", "--app-home"])
        .arg(&home.path)
        .args(["--install-id", r.id.as_str()])
        .env("HERDR_APP_HOME", &home.path)
        .stdin(Stdio::from(lifetime))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let error = format!("cannot start owned writer: {e}");
            return Err(attempt.failed(&home, &r, &error));
        }
    };
    match ready(&mut child, &r.id) {
        Ack::Ready => {}
        Ack::Pending(_) if registered(&home, &r, child.id()) => {}
        Ack::Pending(error) => {
            // Not registered yet: leave it starting, still counted. A later ensure
            // proves it busy (clearing the count) or finds its lock free once it
            // gives up. Nothing is killed; only the final attempt disables.
            return Err(attempt.failed(&home, &r, &format!("{error}; writer still starting")));
        }
        Ack::Exited(error) => {
            // Never signal; quiescence is a fresh description of the lifetime lock.
            let lock = r.lock_file(&home)?;
            let _ = child.try_wait();
            if fs::lock(&lock, STOP_WAIT).is_ok() {
                return Err(attempt.failed(&home, &r, error));
            }
            r.disable(&home)?;
            attempt.clear(&home, &r);
            return Err(format!(
                "{error}; stop timeout; generation disabled; receipt retained"
            ));
        }
        Ack::Foreign => {
            r.disable(&home)?;
            attempt.clear(&home, &r);
            // Cooperate through control; never signal a stored/guessed watcher PID.
            let lock = r.lock_file(&home)?;
            let stopped = fs::lock(&lock, STOP_WAIT);
            let _ = child.try_wait();
            return Err(format!(
                "foreign writer acknowledgement; generation disabled; {}",
                if stopped.is_ok() {
                    "writer stopped"
                } else {
                    "stop timeout; receipt retained"
                }
            ));
        }
    }
    attempt.clear(&home, &r);
    Ok(())
}

pub fn serve(home: Home, id: &str) -> Result<()> {
    let r = Receipt::read(&home)?;
    if r.id != id {
        return Err("writer install generation changed".into());
    }
    r.check_artifact(&home)?;
    r.enabled(&home)?;
    let expected = r.lock_file(&home)?;
    let stdin = std::io::stdin();
    let lifetime: File = rustix::io::dup(stdin.as_fd())
        .map_err(|e| e.to_string())?
        .into();
    use rustix::io::{FdFlags, fcntl_setfd};
    fcntl_setfd(&lifetime, FdFlags::CLOEXEC).map_err(|e| e.to_string())?;
    let a = expected.metadata().map_err(|e| e.to_string())?;
    let b = lifetime.metadata().map_err(|e| e.to_string())?;
    if !b.is_file()
        || b.nlink() != 1
        || b.mode() & 0o7777 != 0o600
        || b.len() != 0
        || (a.dev(), a.ino()) != (b.dev(), b.ino())
    {
        return Err("writer stdin is not the installer lifetime lock".into());
    }
    lifetime
        .try_lock()
        .map_err(|e| format!("writer reservation unavailable: {e}"))?;
    drop(expected);
    super::runtime::watch(home, r, lifetime)
}
