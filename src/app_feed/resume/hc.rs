//! The few hc calls agent resume makes. `send` is the only one that writes to a terminal.
use crate::host::{Core, command};
use serde_json::Value;
use std::{
    ffi::OsString,
    process::{Command, Output},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

const LIMIT: usize = 2 * 1024 * 1024;
const READ: Duration = Duration::from_secs(2);
/// Past the core's ~3 s wait for a person to stop typing.
const SEND: Duration = Duration::from_secs(10);
/// `hc`'s exit code for not_found (`hc --skill`).
const NOT_FOUND: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub core: String,
    pub uptime_ms: u64,
    pub holders: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foreground {
    pub pid: u32,
    pub cwd: String,
}

/// The fields of `hc get` that decide anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub live: bool,
    pub keep: bool,
    pub pid: Option<u32>,
    /// Set while the program is lost or ended; cleared once its shell runs.
    pub reason: Option<String>,
    pub created_ms: u64,
    /// The program's own report. A plain shell has none.
    pub reports: bool,
    pub foreground: Option<Foreground>,
}

impl Term {
    pub fn parse(v: &Value) -> Option<Self> {
        let number = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
        let foreground = v.get("foreground").filter(|f| !f.is_null());
        Some(Self {
            live: v.get("state")?.as_str()? == "live",
            keep: v.get("keep").and_then(Value::as_bool).unwrap_or(false),
            pid: v.get("pid").and_then(number),
            reason: v.get("reason").and_then(Value::as_str).map(str::to_owned),
            created_ms: v.get("created_ms")?.as_u64()?,
            reports: v.get("status").is_some_and(|s| !s.is_null()),
            foreground: match foreground {
                None => None,
                Some(f) => Some(Foreground {
                    pid: number(f.get("pid")?)?,
                    cwd: f.get("cwd")?.as_str()?.to_owned(),
                }),
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Found(Term),
    Gone,
    /// Nothing can be concluded from this read.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    Accepted,
    /// hc's error kind, or `None` when the outcome is unknown. Either way it may have arrived.
    Failed(Option<String>),
}

pub trait Hc {
    fn status(&self) -> Option<Status>;
    fn get(&self, address: &str) -> Lookup;
    fn screen(&self, address: &str) -> Option<String>;
    fn send(&self, address: &str, text: &str) -> Sent;
}

pub struct CoreHc {
    binary: OsString,
    stop: Option<Arc<AtomicBool>>,
}

impl CoreHc {
    pub fn new(core: Core) -> Self {
        Self {
            binary: core.binary().to_owned(),
            stop: core.stop(),
        }
    }
    fn run(&self, args: &[&str], timeout: Duration, stoppable: bool) -> Option<Output> {
        let mut hc = Command::new(&self.binary);
        // A caller's address would make hc wrap the text in a tag.
        hc.args(args).env_remove("HC_TERMINAL_ID");
        let stop = self.stop.as_deref().filter(|_| stoppable);
        command::read(hc, timeout, LIMIT, stop).ok()
    }
    fn json(&self, args: &[&str]) -> Option<Value> {
        let out = self.run(args, READ, true)?;
        out.status
            .success()
            .then(|| serde_json::from_slice(&out.stdout).ok())
            .flatten()
    }
}

impl Hc for CoreHc {
    fn status(&self) -> Option<Status> {
        let v = self.json(&["server", "status", "--json"])?;
        Some(Status {
            core: v["core"]["id"].as_str()?.to_owned(),
            uptime_ms: v["uptime_ms"].as_u64()?,
            holders: v["holders"]
                .as_array()?
                .iter()
                .map(|h| h["instance"].as_str().map(str::to_owned))
                .collect::<Option<_>>()?,
        })
    }
    fn get(&self, address: &str) -> Lookup {
        let Some(out) = self.run(&["get", "--json", address], READ, true) else {
            return Lookup::Unknown;
        };
        if out.status.code() == Some(NOT_FOUND) {
            return Lookup::Gone;
        }
        out.status
            .success()
            .then(|| serde_json::from_slice(&out.stdout).ok())
            .flatten()
            .and_then(|v: Value| Term::parse(&v))
            .map_or(Lookup::Unknown, Lookup::Found)
    }
    fn screen(&self, address: &str) -> Option<String> {
        let v = self.json(&["read", "--json", "--lines", "0", address])?;
        v["text"].as_str().map(str::to_owned)
    }
    /// No `--if idle`: a plain shell has no status, and hc refuses it as "silent, not idle".
    /// `Resume` reads the terminal itself right before this.
    fn send(&self, address: &str, text: &str) -> Sent {
        // Not stoppable: killing the child once started leaves the outcome unknown.
        let Some(out) = self.run(&["send", "--json", address, text], SEND, false) else {
            return Sent::Failed(None);
        };
        if out.status.success() {
            return Sent::Accepted;
        }
        let error: Option<Value> = serde_json::from_slice(&out.stderr).ok();
        let kind = error.and_then(|e| e["error"]["kind"].as_str().map(str::to_owned));
        Sent::Failed(kind)
    }
}
