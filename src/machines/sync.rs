//! One machine's transcript mirror (Python `machines.sync`, `mirror_root`, `_ssh_options`,
//! `machine_lock`): what the scanner reads of the named sessions, mirrored from the machine's
//! `~/.claude/projects` over batch ssh, then the sessions gone longer than the done window
//! pruned. One sync at a time per mirror: two would append the same bytes twice.

use super::mirror::{Rsync, Runner, run};
use super::prune::prune;
use super::{Machine, safe_target};
use herdr_agent_kit::agent_session::SessionId;
use herdr_agent_kit::plugin_env::PluginEnv;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// The error when a machine's ssh target could be read as an option.
pub const UNSAFE_TARGET: &str = "unsafe ssh target";

/// Mirrors a machine's sessions; `None` on success, else the error.
pub trait Syncer: Send + Sync {
    /// Mirror what the scanner reads of `sessions` from `machine`, then prune.
    fn sync(&self, machine: &Machine, sessions: &[String]) -> Option<String>;
}

/// A machine's mirror: `<state dir>/remote/<its id, alphanumerics only>/projects`.
pub fn mirror_root(state_dir: &Path, machine: &Machine) -> PathBuf {
    let id: String = machine.id.chars().filter(|c| c.is_alphanumeric()).collect();
    state_dir.join("remote").join(id).join("projects")
}

/// ssh in batch mode (never prompts), with a connect timeout, keep-alives and a shared
/// control connection under `$XDG_RUNTIME_DIR` (else `/run/user/<uid>`).
pub fn ssh_options(env: &PluginEnv) -> Vec<String> {
    let run = env.xdg_runtime_dir().map_or_else(
        || format!("/run/user/{}", env.uid()),
        |d| d.display().to_string(),
    );
    let mut argv = vec!["ssh".to_owned()];
    for opt in [
        "BatchMode=yes".to_owned(),
        "ConnectTimeout=5".to_owned(),
        "ServerAliveInterval=5".to_owned(),
        "ControlMaster=auto".to_owned(),
        format!("ControlPath={run}/panorama-ssh-%C"),
        "ControlPersist=60".to_owned(),
    ] {
        argv.extend(["-o".to_owned(), opt]);
    }
    argv
}

/// The lock of one mirror, shared by every sync in this process. Keyed by the mirror's
/// directory, not the machine id: two ids that differ only in other characters than
/// alphanumerics (`a-b`, `ab`) share one mirror, so they share its lock too. (Python keys it
/// by the machine id: issue #87.)
pub fn mirror_lock(mirror: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(Mutex::default);
    let mut locks = locks.lock().unwrap_or_else(PoisonError::into_inner);
    locks.entry(mirror.to_path_buf()).or_default().clone()
}

/// The canonical session ids, each once, in order; anything else dropped.
fn canonical(sessions: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for sid in sessions.iter().filter_map(|s| SessionId::parse(s)) {
        if !out.iter().any(|s| s == sid.as_str()) {
            out.push(sid.as_str().to_owned());
        }
    }
    out
}

/// The real sync: rsync over ssh into the state dir.
pub struct RsyncSync {
    /// panorama's state dir (the mirrors live under it).
    pub state_dir: PathBuf,
    /// How the passes run (the ssh options among them).
    pub rsync: Rsync,
    /// Runs one rsync argv ([`run`] outside tests).
    pub runner: Arc<Runner>,
    /// The done window now, in seconds (it follows the config).
    pub keep: Arc<dyn Fn() -> f64 + Send + Sync>,
}

impl RsyncSync {
    /// From the plugin environment, with `keep` giving the done window.
    pub fn new(env: &PluginEnv, keep: Arc<dyn Fn() -> f64 + Send + Sync>) -> Self {
        Self {
            state_dir: herdr_agent_kit::internal::plugin_paths::panorama::state_dir(env),
            rsync: Rsync::new(Some(ssh_options(env))),
            runner: Arc::new(run),
            keep,
        }
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

impl Syncer for RsyncSync {
    fn sync(&self, machine: &Machine, sessions: &[String]) -> Option<String> {
        let dest = mirror_root(&self.state_dir, machine);
        let _ = std::fs::create_dir_all(&dest);
        let sessions = canonical(sessions);
        let lock = mirror_lock(&dest);
        let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut err = None;
        if !sessions.is_empty() {
            if !safe_target(&machine.target) {
                return Some(UNSAFE_TARGET.to_owned());
            }
            let source = format!("{}:.claude/projects/", machine.target);
            err = self.rsync.mirror(&source, &dest, &sessions, &*self.runner);
        }
        prune(&dest, &sessions, (self.keep)(), now());
        err
    }
}

#[cfg(test)]
mod tests;
