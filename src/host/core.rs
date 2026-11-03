//! Bounded, read-only hc commands. No shell and no caller means no implicit child-seen action.
use super::context::{pane_key, valid_address};
use herdr_agent_kit::{agent_procs::ProcessTable, herdr_api::ProcessInfo};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct Core {
    binary: OsString,
    timeout: Duration,
    stop: Option<Arc<AtomicBool>>,
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
            stop: self.stop.clone(),
        }
    }
    pub fn new(binary: OsString, timeout: Duration) -> Self {
        Self {
            binary,
            timeout,
            stop: None,
        }
    }
    pub fn with_stop(mut self, stop: Arc<AtomicBool>) -> Self {
        self.stop = Some(stop);
        self
    }
    pub fn binary(&self) -> &std::ffi::OsStr {
        &self.binary
    }
    pub fn read(&self, args: &[&str]) -> Result<Value, &'static str> {
        let mut command = Command::new(&self.binary);
        command.args(args).env_remove("HC_TERMINAL_ID");
        let output = super::command::read(
            command,
            self.timeout,
            MAX_BYTES as usize,
            self.stop.as_deref(),
        )
        .map_err(|e| match e {
            "helper unavailable" => "hc unavailable",
            "helper stdout unavailable" | "helper stderr unavailable" => "hc output unavailable",
            "helper output setup failed" => "hc output setup failed",
            "helper output unreadable" => "hc output unreadable",
            "helper output exceeds limit" => "hc output exceeds limit",
            "helper wait failed" => "hc wait failed",
            "helper read timed out" => "hc read timed out",
            _ => "hc read cancelled",
        })?;
        if !output.status.success() {
            return Err("hc read failed");
        }
        serde_json::from_slice(&output.stdout).map_err(|_| "hc output is not JSON")
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
