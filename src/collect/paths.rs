//! Where the collector reads beside the transcripts (Python `events.events_dir`,
//! `hooks.hooks_dir`): the explicit thread channel and the hook records, each under
//! `$XDG_STATE_HOME/herdr-panorama` unless its own variable names a directory.

use herdr_agent_kit::agent_session::SessionId;
use herdr_agent_kit::plugin_env::PluginEnv;
use std::path::{Path, PathBuf};

fn under_state(env: &PluginEnv, var: &str, leaf: &str) -> PathBuf {
    match env.path_var(var) {
        Some(dir) => dir.to_path_buf(),
        None => env.xdg_state_home().join("herdr-panorama").join(leaf),
    }
}

/// `$PANORAMA_EVENTS_DIR`, else `$XDG_STATE_HOME/herdr-panorama/events`.
pub fn events_dir(env: &PluginEnv) -> PathBuf {
    under_state(env, "PANORAMA_EVENTS_DIR", "events")
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

/// A session's events file: its canonical id, else its safe characters.
pub fn events_path(dir: &Path, session_id: &str) -> PathBuf {
    let name = SessionId::parse(session_id)
        .map_or_else(|| safe_name(session_id), |id| id.as_str().to_owned());
    dir.join(format!("{name}.jsonl"))
}

/// A session's hook records: `<safe id>.jsonl`.
pub fn hooks_path(dir: &Path, session_id: &str) -> PathBuf {
    dir.join(format!("{}.jsonl", safe_name(session_id)))
}

#[cfg(test)]
mod tests;
