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

/// A transient failure with no running writer: count it, back off, and disable
/// only once the bound is reached or the count cannot be recorded.
fn transient(home: &Home, r: &Receipt, ledger: Option<&(Ledger, Known)>, error: &str) -> String {
    let failures = ledger.map_or(0, |(l, _)| l.failures) + 1;
    if failures >= MAX_FAILURES {
        let disabled = r.disable(home);
        return format!(
            "{error}; {failures} consecutive transient start failures; generation disabled: {disabled:?}"
        );
    }
    match Ledger::record(home, r, failures, ledger.map(|(_, k)| k)) {
        Ok(wait) => format!(
            "{error}; transient start failure {failures}/{MAX_FAILURES}; retry after {wait} ms"
        ),
        Err(e) => {
            let disabled = r.disable(home);
            format!("{error}; start ledger unavailable ({e}); generation disabled: {disabled:?}")
        }
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
                let _ = home.remove(&r.path(LEDGER), &known);
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
            return Err(transient(&home, &r, ledger.as_ref(), &error));
        }
    };
    match ready(&mut child, &r.id) {
        Ack::Ready => {}
        Ack::Pending(_) if registered(&home, &r, child.id()) => {}
        Ack::Pending(error) => {
            // Not registered yet: leave it starting. A later ensure proves it busy,
            // or finds its lock free once it gives up. Nothing is disabled or killed.
            return Err(format!("{error}; writer still starting; retry later"));
        }
        Ack::Exited(error) => {
            // Never signal; quiescence is a fresh description of the lifetime lock.
            let lock = r.lock_file(&home)?;
            let _ = child.try_wait();
            if fs::lock(&lock, STOP_WAIT).is_ok() {
                return Err(transient(&home, &r, ledger.as_ref(), error));
            }
            r.disable(&home)?;
            return Err(format!(
                "{error}; stop timeout; generation disabled; receipt retained"
            ));
        }
        Ack::Foreign => {
            r.disable(&home)?;
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
    if let Some((_, known)) = &ledger {
        // Advisory reset after a proven start; a changed ledger is left for review.
        let _ = home.remove(&r.path(LEDGER), known);
    }
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
