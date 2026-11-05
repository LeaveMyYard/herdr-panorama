//! Which conversation a core terminal's foreground agent process runs (session-linking.md).
//! Every collect recomputes it; nothing here is persisted.
mod claude;
mod decide;
mod proc;

pub use decide::{Link, combine, unique};
pub use proc::{Environment, Proc};

use super::records::{self, Hooks};
use herdr_agent_kit::{
    agent_procs::{ProcessTable, claude::first_claude},
    agent_session::claude::io::ProjectsRoot,
    plugin_env::PluginEnv,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// One piece of evidence about a terminal's session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    Absent,
    Ambiguous,
    Names(String),
    /// Several hook sessions for one process: ambiguous unless another signal names this,
    /// the newest of them (§3 H).
    Newest(String),
}

pub struct Linker {
    pub proc: Proc,
    pub uid: u32,
    /// The transcript root the collector looks every linked id up in.
    pub projects: Option<PathBuf>,
}

impl Linker {
    pub fn system(env: &PluginEnv) -> Self {
        Self {
            proc: Proc::new("/proc", env.uid()),
            uid: env.uid(),
            projects: ProjectsRoot::resolve(
                env.path_var("CLAUDE_CONFIG_DIR").map(Into::into),
                env.home().map(Into::into),
            )
            .map(|root| root.path().to_path_buf()),
        }
    }

    /// The decision for one live local row (`info` is `core::process` of `hc get`).
    pub fn terminal(
        &self,
        env: &PluginEnv,
        address: &str,
        harness: &str,
        info: &Value,
        hooks: &Hooks,
        table: &dyn ProcessTable,
    ) -> (Option<u32>, Link) {
        let pid = foreground(info).map(|f| f.0);
        let signals = if harness == "claude" {
            vec![
                records::claude_session(hooks, address, info, table),
                self.per_process(address, info, table),
            ]
        } else {
            let bound = crate::codex::identity::resolve_core(env, address, info, table);
            vec![bound.map_or(Signal::Absent, Signal::Names)]
        };
        (pid, combine(&signals))
    }

    /// Evidence P behind the §2 process gate.
    pub fn per_process(&self, address: &str, info: &Value, table: &dyn ProcessTable) -> Signal {
        let Some((pid, name, argv)) = foreground(info) else {
            return Signal::Absent;
        };
        if address.contains('/') {
            return Signal::Absent;
        }
        let Some(before) = table.start(pid) else {
            return Signal::Absent;
        };
        let comm = table.stat(pid).map_or(String::new(), |s| s.comm);
        let cmdline = table.cmdline(pid);
        let env = self.proc.environment(pid);
        if first_claude([(pid, comm.as_str(), cmdline.as_slice())]).is_none()
            || name.as_deref() != Some(comm.as_str())
            || argv.map(|a| a.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>())
                != Some(cmdline)
            || !self.proc.owned(pid)
            || env.as_ref().and_then(|e| e.terminal.as_deref()) != Some(address)
        {
            return Signal::Absent;
        }
        // The record's location comes from the process; its proof is inside the record (§2.6).
        let Some(config) = env
            .as_ref()
            .and_then(Environment::claude_dir)
            .filter(|dir| self.shares_transcripts(dir))
        else {
            return Signal::Absent;
        };
        let signal = claude::record(&config, pid, &before, self.uid);
        if table.start(pid).as_deref() != Some(before.as_str()) {
            return Signal::Absent;
        }
        signal
    }
}

impl Linker {
    /// `config`'s transcripts are the collector's, links resolved. Otherwise an id of this
    /// process could find a same-named transcript it does not write.
    fn shares_transcripts(&self, config: &Path) -> bool {
        let resolved = |path: &Path| std::fs::canonicalize(path).ok();
        resolved(&config.join("projects"))
            .is_some_and(|own| self.projects.as_deref().and_then(resolved) == Some(own))
    }
}

/// The single foreground process `hc get` reported (core::process builds exactly one):
/// its PID, `name` and `argv`. hc 0.1.17 fills `name` with `/proc` `comm` and `argv` with
/// the NUL-split cmdline (tests/fixtures/hc-get-foreground.json); the gate compares `name`
/// with `comm` exactly and `argv`, empty arguments dropped as the kit drops them, with
/// `cmdline`.
fn foreground(info: &Value) -> Option<(u32, Option<String>, Option<Vec<String>>)> {
    let fg = &info["process_info"]["foreground_processes"][0];
    let pid = u32::try_from(fg["pid"].as_u64()?).ok().filter(|p| *p > 1)?;
    let name = fg["name"].as_str().map(str::to_owned);
    let argv = fg["argv"].as_array().and_then(|a| {
        a.iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
    });
    Some((pid, name, argv))
}

#[cfg(test)]
mod tests;
