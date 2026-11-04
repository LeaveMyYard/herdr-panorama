//! Where the collector reads beside the transcripts (Python `hooks.hooks_dir`): the hook
//! records, under `$XDG_STATE_HOME/herdr-panorama` unless `$PANORAMA_HOOKS_DIR` names a
//! directory. The explicit thread channel's `events` dir is no longer read, and
//! `$PANORAMA_EVENTS_DIR` is ignored (plan `uniform-agent-tree.md` §6).

use herdr_agent_kit::plugin_env::PluginEnv;
use std::path::{Path, PathBuf};

fn under_state(env: &PluginEnv, var: &str, leaf: &str) -> PathBuf {
    match env.path_var(var) {
        Some(dir) => dir.to_path_buf(),
        None => env.xdg_state_home().join("herdr-panorama").join(leaf),
    }
}

/// `$PANORAMA_HOOKS_DIR`, else `$XDG_STATE_HOME/herdr-panorama/hooks`.
pub fn hooks_dir(env: &PluginEnv) -> PathBuf {
    under_state(env, "PANORAMA_HOOKS_DIR", "hooks")
}

/// Only alphanumerics, `-` and `_` (Python's `isalnum() or c in "-_"`).
pub fn safe_name(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// A session's hook records: `<safe id>.jsonl`.
pub fn hooks_path(dir: &Path, session_id: &str) -> PathBuf {
    dir.join(format!("{}.jsonl", safe_name(session_id)))
}

#[cfg(test)]
mod tests;
