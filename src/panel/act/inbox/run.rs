//! Running the inbox's own CLI from its plugin root (Python `inbox_link._inbox`, `_env`):
//! `uv run … --project <root> inbox <args>`, with the inbox's plugin id and root and none of
//! panorama's invocation context (which would make it act on panorama's pane).

use herdr_agent_kit::herdr_api::{MAX_ARG, clean};
use std::time::Duration;
use tokio::process::Command;

/// How long the inbox's CLI may take.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// panorama's own invocation context: never passed to the inbox.
const STRIP: [&str; 7] = [
    "HERDR_PLUGIN_CONTEXT_JSON",
    "HERDR_PANE_ID",
    "HERDR_TAB_ID",
    "HERDR_WORKSPACE_ID",
    "HERDR_PLUGIN_CONFIG_DIR",
    "HERDR_PLUGIN_STATE_DIR",
    "HERDR_PLUGIN_ENTRYPOINT_ID",
];

/// How a run ended: its exit code (124 a timeout, 127 not started) and its output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub code: i32,
    pub out: String,
}

/// Runs the inbox's CLI (a test stands in for it).
pub trait Runner {
    /// `inbox <args>` from the plugin root `root`.
    fn inbox(&self, root: &str, args: &[&str]) -> impl Future<Output = Ran>;
}

/// The real one: `uv run` in the inbox's project.
#[derive(Debug, Default)]
pub struct UvRunner;

/// The command that runs `inbox <args>` from the plugin root `root`: `uv run` in the
/// inbox's project, with the inbox's plugin id and root, without `UV_*`, `VIRTUAL_ENV*` or
/// panorama's own invocation context (`STRIP`), which would make it act on panorama's pane.
pub fn command(root: &str, args: &[&str], env: impl IntoIterator<Item = String>) -> Command {
    let mut cmd = Command::new("uv");
    cmd.args([
        "run",
        "--quiet",
        "--frozen",
        "--no-dev",
        "--project",
        root,
        "inbox",
    ]);
    cmd.args(args.iter().map(|a| clean(a, MAX_ARG)));
    cmd.current_dir(root).kill_on_drop(true);
    for k in env {
        if k.starts_with("UV_") || k.starts_with("VIRTUAL_ENV") || STRIP.contains(&k.as_str()) {
            cmd.env_remove(&k);
        }
    }
    cmd.env("HERDR_PLUGIN_ID", super::INBOX_ID)
        .env("HERDR_PLUGIN_ROOT", root);
    cmd
}

impl Runner for UvRunner {
    async fn inbox(&self, root: &str, args: &[&str]) -> Ran {
        let env = std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned());
        let mut cmd = command(root, args, env);
        match tokio::time::timeout(TIMEOUT, cmd.output()).await {
            // it may have opened the popup: no fallback
            Err(_) => Ran {
                code: 124,
                out: "timeout".into(),
            },
            Ok(Err(e)) => Ran {
                code: 127,
                out: format!("Failed to spawn: {e}"),
            },
            Ok(Ok(o)) => Ran {
                code: o.status.code().unwrap_or(1),
                out: format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ),
            },
        }
    }
}
