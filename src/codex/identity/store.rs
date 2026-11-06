//! Plugin-owned associations; native herdr identity always takes precedence.
use super::{files::Directory, owner, valid_thread_id};
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    herdr_api::ProcessInfo,
    plugin_env::PluginEnv,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs::File, io::Read};
const MAX_BYTES: u64 = 4096;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Association {
    pub version: u8,
    pub pane_id: String,
    pub thread_id: String,
    pub owner_pid: u32,
    pub owner_start: String,
    pub embedded: bool,
    pub conflicted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pidns: Option<String>,
}
impl Association {
    fn valid(&self, pane: &str) -> bool {
        self.version == 1
            && self.pane_id == pane
            && valid_thread_id(&self.thread_id)
            && self.owner_pid > 1
            && !self.owner_start.is_empty()
            && self.owner_start.len() <= 128
    }
}
fn name(pane: &str) -> Option<String> {
    if pane.is_empty() || pane.len() > 100 {
        return None;
    }
    Some(pane.as_bytes().iter().map(|b| format!("{b:02x}")).collect())
}
fn read(file: File, pane: &str) -> Option<Association> {
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > MAX_BYTES as usize {
        return None;
    }
    let association: Association = serde_json::from_slice(&bytes).ok()?;
    association.valid(pane).then_some(association)
}
/// Native exact identity wins; the process callback is needed only for a valid local fallback.
/// `process` returns the raw `pane.process_info` result, including its `process_info` object.
pub fn resolve(
    env: &PluginEnv,
    pane_id: &str,
    native_id: Option<&str>,
    process: impl FnOnce() -> Option<Value>,
) -> Option<String> {
    resolve_with(env, pane_id, native_id, process, &SystemProcesses::new())
}
pub(super) fn resolve_with(
    env: &PluginEnv,
    pane_id: &str,
    native_id: Option<&str>,
    process: impl FnOnce() -> Option<Value>,
    table: &dyn ProcessTable,
) -> Option<String> {
    if let Some(id) = native_id {
        return valid_thread_id(id).then(|| id.to_owned());
    }
    let dir = Directory::open(env, false).ok()?;
    let a = read(dir.file(&name(pane_id)?, false).ok()?, pane_id)?;
    if a.conflicted || a.host.is_some() || a.terminal.is_some() || a.pidns.is_some() {
        return None;
    }
    let info: ProcessInfo = serde_json::from_value(process()?.get("process_info")?.clone()).ok()?;
    if !owner::matches(
        &info,
        pane_id,
        &owner::Owner {
            pid: a.owner_pid,
            start: a.owner_start.clone(),
        },
        a.embedded,
        table,
    ) {
        return None;
    }
    Some(a.thread_id)
}
pub(super) struct Store {
    dir: Directory,
    name: String,
    _lock: File,
}
impl Store {
    pub fn lock(env: &PluginEnv, pane: &str) -> Result<Self, &'static str> {
        let name = name(pane).ok_or("invalid pane ID")?;
        Self::lock_named(env, name)
    }
    pub fn core_lock(env: &PluginEnv, address: &str) -> Result<Self, &'static str> {
        Self::lock_named(env, core_name(address).ok_or("invalid core address")?)
    }
    fn lock_named(env: &PluginEnv, name: String) -> Result<Self, &'static str> {
        let dir =
            Directory::open(env, true).map_err(|_| "cannot open private binding directory")?;
        let lock = dir
            .file(&format!("{name}.lock"), true)
            .map_err(|_| "cannot open private binding lock")?;
        lock.try_lock()
            .map_err(|_| "another pane registration is in progress; retry")?;
        Ok(Self {
            dir,
            name,
            _lock: lock,
        })
    }
    pub fn existing(&self, pane: &str) -> Result<Option<Association>, &'static str> {
        match self.dir.file(&self.name, false) {
            Ok(file) => read(file, pane)
                .map(Some)
                .ok_or("invalid existing binding; use codex-bind THREAD_ID --pane PANE_ID"),
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(_) => Err("cannot read private existing binding"),
        }
    }
    pub fn write(&self, value: &Association) -> Result<(), &'static str> {
        let bytes = serde_json::to_vec(value).map_err(|_| "cannot encode binding")?;
        if bytes.len() > MAX_BYTES as usize {
            return Err("binding exceeds size limit");
        }
        self.dir.replace(&self.name, &bytes)
    }
}

fn core_name(address: &str) -> Option<String> {
    crate::host::context::valid_address(address).then(|| format!("core-{}", name(address).unwrap()))
}

pub(super) fn core_existing(env: &PluginEnv, address: &str) -> Option<Association> {
    let dir = Directory::open(env, false).ok()?;
    let association = read(
        dir.file(&core_name(address)?, false).ok()?,
        &crate::host::context::pane_key(address),
    )?;
    (association.host.as_deref() == Some("herdr-core")
        && association.terminal.as_deref() == Some(address))
    .then_some(association)
}
