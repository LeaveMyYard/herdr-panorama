//! The real `hc` and `/proc`. `send` is the only call that writes to a terminal.
use super::{
    hc::{Hc, Lookup, Paint, Sent, Status, Term},
    state::Config,
};
use crate::host::{Core, command, link::Signal, link::claude};
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    plugin_env::PluginEnv,
};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

const LIMIT: usize = 2 * 1024 * 1024;
const READ: Duration = Duration::from_secs(2);
/// Past the core's ~3 s wait for a person to stop typing.
const SEND: Duration = Duration::from_secs(10);
/// A probe of an untouched terminal answers in milliseconds.
const QUIET: Duration = Duration::from_millis(500);
const PAINT_GAP: Duration = Duration::from_millis(120);
/// `hc`'s exit code for not_found (`hc --skill`).
const NOT_FOUND: i32 = 3;

pub struct CoreHc {
    binary: OsString,
    stop: Option<Arc<AtomicBool>>,
    home: Option<PathBuf>,
    uid: u32,
}

impl CoreHc {
    pub fn new(core: Core, env: &PluginEnv) -> Self {
        Self {
            binary: core.binary().to_owned(),
            stop: core.stop(),
            home: env.home().map(Path::to_path_buf),
            uid: env.uid(),
        }
    }
    fn dirs(&self) -> Vec<PathBuf> {
        self.home.as_deref().map_or(Vec::new(), claude::config_dirs)
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
    fn paint(&self, address: &str) -> Option<Paint> {
        let spawned = Command::new(&self.binary)
            .args(["attach", "--observe", address])
            .env_remove("HC_TERMINAL_ID")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = spawned.ok()?;
        let mut output = child.stdout.take()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut chunk = [0_u8; 65536];
            while let Ok(n) = output.read(&mut chunk) {
                if n == 0 || tx.send(chunk[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        // It never ends by itself: a paint is whatever came before a short silence.
        let mut bytes = rx.recv_timeout(QUIET).unwrap_or_default();
        while let Ok(more) = rx.recv_timeout(PAINT_GAP) {
            bytes.extend(more);
        }
        let _ = child.kill();
        let _ = child.wait();
        Paint::parse(&bytes)
    }
    fn children(&self, pid: u32) -> bool {
        let path = format!("/proc/{pid}/task/{pid}/children");
        // Elsewhere there is no such file, and a loss there always waits for a person.
        std::fs::read_to_string(path)
            .map_or(cfg!(target_os = "linux"), |text| !text.trim().is_empty())
    }
    fn config(&self, pid: u32) -> Config {
        let table = SystemProcesses::new();
        let Some(start) = table.start(pid) else {
            return Config::Unproved;
        };
        let held: Vec<PathBuf> = self
            .dirs()
            .into_iter()
            .filter(|dir| matches!(claude::record(dir, pid, &start, self.uid), Signal::Names(_)))
            .collect();
        match (held.as_slice(), self.home.as_deref()) {
            ([dir], Some(home)) if *dir == home.join(".claude") => Config::Default,
            ([dir], _) => dir
                .to_str()
                .map_or(Config::Unproved, |d| Config::Named(d.into())),
            _ => Config::Unproved,
        }
    }
    fn held(&self, session: &str) -> bool {
        let table = SystemProcesses::new();
        claude::held(&self.dirs(), session, &|pid| table.start(pid))
    }
    /// `terminal.keys` with no keys: it waits out the same person-typing gate as `send`, then
    /// does nothing. A prompt answer means `send` will not wait; no answer means someone is
    /// typing. This is the only way to bound `send`, which cannot be cancelled once started.
    fn quiet(&self, address: &str) -> bool {
        let call = json!({"jsonrpc":"2.0","id":1,"method":"terminal.keys",
            "params":{"terminal":address,"keys":[]}});
        let spawned = Command::new(&self.binary)
            .arg("connect")
            .env_remove("HC_TERMINAL_ID")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else {
            return false;
        };
        let (input, output) = (child.stdin.take(), child.stdout.take());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (Some(mut input), Some(output)) = (input, output) else {
                return;
            };
            let mut line = String::new();
            if writeln!(input, "{call}").is_ok() {
                let _ = BufReader::new(output).read_line(&mut line);
            }
            let _ = tx.send(line);
        });
        let line = rx.recv_timeout(QUIET).unwrap_or_default();
        let _ = child.kill();
        let _ = child.wait();
        let answer: Option<Value> = serde_json::from_str(&line).ok();
        answer.is_some_and(|a| a.get("result").is_some())
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
