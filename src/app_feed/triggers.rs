//! Events and observed-file metadata are refresh triggers, never identity evidence.
use super::{World, fs::Result};
use crate::host::Core;
use std::{
    fs,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

struct Events {
    child: Child,
    stdout: ChildStdout,
}
impl Events {
    fn start(core: &Core) -> Result<Self> {
        let mut child = Command::new(core.binary())
            .args(["events", "--json"])
            .env_remove("HC_TERMINAL_ID")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or("core events stdout unavailable")?;
        use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
        if fcntl_getfl(&stdout)
            .and_then(|f| fcntl_setfl(&stdout, f | OFlags::NONBLOCK))
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("core events setup failed".into());
        }
        Ok(Self { child, stdout })
    }
    fn changed(&mut self) -> Result<bool> {
        let mut bytes = [0_u8; 65536];
        match self.stdout.read(&mut bytes) {
            Ok(0) => Err("core events ended".into()),
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(_) => Err("core events unreadable".into()),
        }
    }
}
impl Drop for Events {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

type Stamp = Vec<(PathBuf, Option<(u64, Option<SystemTime>)>)>;
pub struct Triggers {
    core: Core,
    events: Option<Events>,
    retry: Instant,
    paths: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    stamp: Stamp,
    pub event_error: bool,
}
fn metadata(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    fs::metadata(path)
        .ok()
        .map(|m| (m.len(), m.modified().ok()))
}
impl Triggers {
    pub fn new(core: Core, dirs: Vec<PathBuf>) -> Self {
        Self {
            core,
            events: None,
            retry: Instant::now(),
            paths: Vec::new(),
            dirs,
            stamp: Vec::new(),
            event_error: false,
        }
    }
    fn signature(&self) -> Stamp {
        let mut out: Stamp = self
            .paths
            .iter()
            .chain(&self.dirs)
            .map(|p| (p.clone(), metadata(p)))
            .collect();
        for dir in &self.dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.take(512).flatten() {
                    let p = entry.path();
                    out.push((p.clone(), metadata(&p)));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
    pub fn observe(&mut self, world: &World) {
        self.paths = world
            .swarm
            .roots
            .iter()
            .flat_map(|o| {
                o.transcript
                    .iter()
                    .chain(o.agents.iter().filter_map(|a| a.transcript.as_ref()))
            })
            .map(PathBuf::from)
            .collect();
        self.paths.sort();
        self.paths.dedup();
        self.stamp = self.signature();
    }
    pub fn pause(&mut self) {
        self.events = None;
    }
    pub fn changed(&mut self) -> bool {
        if self.events.is_none() && Instant::now() >= self.retry {
            self.events = Events::start(&self.core).ok();
            self.event_error = self.events.is_none();
            self.retry = Instant::now() + Duration::from_secs(5);
        }
        let event = match self.events.as_mut().map(Events::changed) {
            Some(Ok(v)) => v,
            Some(Err(_)) => {
                self.events = None;
                self.event_error = true;
                false
            }
            None => false,
        };
        let next = self.signature();
        let file = next != self.stamp;
        self.stamp = next;
        event || file
    }
}
