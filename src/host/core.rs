//! Bounded, read-only hc commands. No shell and no caller means no implicit child-seen action.
use super::context::{pane_key, valid_address};
use herdr_agent_kit::{agent_procs::ProcessTable, herdr_api::ProcessInfo};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct Core {
    binary: OsString,
    timeout: Duration,
}
impl Default for Core {
    fn default() -> Self {
        Self::new("hc".into(), Duration::from_secs(2))
    }
}
impl Core {
    pub fn with_timeout(&self, timeout: Duration) -> Self {
        Self {
            binary: self.binary.clone(),
            timeout,
        }
    }
    pub fn new(binary: OsString, timeout: Duration) -> Self {
        Self { binary, timeout }
    }
    pub fn read(&self, args: &[&str]) -> Result<Value, &'static str> {
        let mut child = Command::new(&self.binary)
            .args(args)
            .env_remove("HC_TERMINAL_ID")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|_| "hc unavailable")?;
        let mut stdout = child.stdout.take().ok_or("hc output unavailable")?;
        use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
        if fcntl_getfl(&stdout)
            .and_then(|flags| fcntl_setfl(&stdout, flags | OFlags::NONBLOCK))
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("hc output setup failed");
        }
        let deadline = Instant::now() + self.timeout;
        let mut bytes = Vec::new();
        let mut eof = false;
        let result = loop {
            let mut chunk = [0_u8; 8192];
            match stdout.read(&mut chunk) {
                Ok(0) => eof = true,
                Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break Err("hc output unreadable"),
            }
            if bytes.len() > MAX_BYTES as usize {
                break Err("hc output exceeds limit");
            }
            match child.try_wait() {
                Ok(Some(status)) if !status.success() => break Err("hc read failed"),
                Ok(Some(_)) if eof => break Ok(()),
                Err(_) => break Err("hc wait failed"),
                _ if Instant::now() >= deadline => break Err("hc read timed out"),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        };
        if result.is_err() {
            let _ = child.kill();
        }
        let _ = child.wait();
        result?;
        if bytes.len() > MAX_BYTES as usize {
            return Err("hc output exceeds limit");
        }
        serde_json::from_slice(&bytes).map_err(|_| "hc output is not JSON")
    }
    pub fn list(&self) -> Result<Value, &'static str> {
        self.read(&["ls", "--json"])
    }
    pub fn terminal(&self, address: &str) -> Result<Value, &'static str> {
        if !valid_address(address) {
            return Err("invalid core address");
        }
        self.read(&["get", "--json", address])
    }
    /// Remote PIDs cannot prove ownership on this machine.
    pub fn process(&self, address: &str) -> Option<Value> {
        if address.contains('/') {
            return None;
        }
        process(&self.terminal(address).ok()?, address)
    }
}

/// The established owner verifier consumes ProcessInfo; preserve its exact PID checks.
pub fn process(terminal: &Value, address: &str) -> Option<Value> {
    if terminal["address"].as_str()? != address || terminal["state"] != "live" {
        return None;
    }
    let fg = terminal.get("foreground")?.as_object()?;
    let pid = fg.get("pid")?.as_u64()?;
    if pid <= 1 || pid > u64::from(u32::MAX) {
        return None;
    }
    Some(
        json!({"process_info":{"pane_id":pane_key(address),"foreground_processes":[{
            "pid":pid,"name":fg.get("name"),"argv":fg.get("argv")
        }]}}),
    )
}

pub fn owns(info: &Value, pane: &str, pid: u32, start: &str, table: &dyn ProcessTable) -> bool {
    let parsed: Result<ProcessInfo, _> = serde_json::from_value(info["process_info"].clone());
    parsed.is_ok_and(|p| {
        p.pane_id.as_ref().is_some_and(|id| id.as_str() == pane)
            && p.foreground_processes
                .as_ref()
                .is_some_and(|ps| ps.iter().any(|p| p.pid == Some(i64::from(pid))))
            && table.start(pid).as_deref() == Some(start)
    })
}

#[cfg(test)]
mod tests;
