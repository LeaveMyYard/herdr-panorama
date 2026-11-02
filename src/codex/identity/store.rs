//! Plugin-owned associations; native herdr identity always takes precedence.
use super::{owner, valid_thread_id};
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    herdr_api::ProcessInfo,
    plugin_env::PluginEnv,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
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
fn path(env: &PluginEnv, pane: &str) -> Option<PathBuf> {
    if pane.is_empty() || pane.len() > 100 {
        return None;
    }
    let name = pane
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Some(
        env.xdg_state_home()
            .join("herdr-panorama/codex-bindings")
            .join(name),
    )
}
fn read(path: &Path, pane: &str) -> Option<Association> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
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
    let a = read(&path(env, pane_id)?, pane_id)?;
    if a.conflicted {
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
    path: PathBuf,
    _lock: File,
}
impl Store {
    pub fn lock(env: &PluginEnv, pane: &str) -> Result<Self, &'static str> {
        let path = path(env, pane).ok_or("invalid pane ID")?;
        let dir = path.parent().ok_or("invalid binding path")?;
        herdr_agent_kit::plugin_env::private_dir::make_private_dir(dir, env.uid())
            .map_err(|_| "cannot create private binding directory")?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path.with_extension("lock"))
            .map_err(|_| "cannot open binding lock")?;
        lock.try_lock()
            .map_err(|_| "another pane registration is in progress; retry")?;
        Ok(Self { path, _lock: lock })
    }
    pub fn existing(&self, pane: &str) -> Result<Option<Association>, &'static str> {
        if !self
            .path
            .try_exists()
            .map_err(|_| "cannot inspect existing binding")?
        {
            return Ok(None);
        }
        read(&self.path, pane)
            .map(Some)
            .ok_or("invalid existing binding; use codex-bind THREAD_ID --pane PANE_ID")
    }
    pub fn write(&self, value: &Association) -> Result<(), &'static str> {
        let bytes = serde_json::to_vec(value).map_err(|_| "cannot encode binding")?;
        if bytes.len() > MAX_BYTES as usize {
            return Err("binding exceeds size limit");
        }
        let temp = self.path.with_extension("pending");
        let result = (|| {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .custom_flags(
                    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
                )
                .open(&temp)
                .map_err(|_| "cannot create pending binding")?;
            file.write_all(&bytes).map_err(|_| "cannot write binding")?;
            file.sync_all().map_err(|_| "cannot persist binding")?;
            std::fs::rename(&temp, &self.path).map_err(|_| "cannot replace binding")
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }
}
