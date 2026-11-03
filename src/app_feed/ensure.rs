//! Startup is serialized with installation; only a reserved lifetime FD reaches the child.
use super::{
    fs::{self, Home, Result},
    journal::Journal,
    receipt::{BINARY, Ownership, Receipt},
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

fn ready(child: &mut Child, id: &str) -> Result<()> {
    let mut stdout = child
        .stdout
        .take()
        .ok_or("writer acknowledgement unavailable")?;
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    fcntl_getfl(&stdout)
        .and_then(|f| fcntl_setfl(&stdout, f | OFlags::NONBLOCK))
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0_u8; 256];
        match stdout.read(&mut chunk) {
            Ok(0) => return Err("writer exited before acknowledgement".into()),
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.to_string()),
        }
        if bytes.ends_with(b"\n") {
            return if bytes == format!("READY {id}\n").as_bytes() {
                Ok(())
            } else {
                Err("foreign writer acknowledgement".into())
            };
        }
        if bytes.len() > 256 || Instant::now() >= deadline {
            return Err("writer acknowledgement timed out".into());
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("writer exited during startup".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn ensure(home: Home) -> Result<()> {
    fs::lock(&home.fd, Duration::from_secs(2))?;
    let r = Receipt::read(&home)?;
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
            return Ok(());
        }
        Err(e) => return Err(format!("writer lock failed: {e}")),
    }
    let mut child = Command::new(home.path.join(BINARY))
        .args(["app-feed", "--watch", "--app-home"])
        .arg(&home.path)
        .args(["--install-id", r.id.as_str()])
        .env("HERDR_APP_HOME", &home.path)
        .stdin(Stdio::from(lifetime))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| {
            let disabled = r.disable(&home);
            format!("cannot start owned writer: {e}; generation stop: {disabled:?}")
        })?;
    if let Err(error) = ready(&mut child, &r.id) {
        r.disable(&home)?;
        // Cooperate through control; never signal a stored/guessed watcher PID.
        let lock = r.lock_file(&home)?;
        let stopped = fs::lock(&lock, Duration::from_secs(2));
        let _ = child.try_wait();
        return Err(format!(
            "{error}; generation disabled; {}",
            if stopped.is_ok() {
                "writer stopped"
            } else {
                "stop timeout; receipt retained"
            }
        ));
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
