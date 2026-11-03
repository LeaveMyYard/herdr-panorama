//! Run only the registered plugin's already-installed executable; never resolve PATH inbox.
use crate::{
    host::command,
    panel::act::inbox::{INBOX_ID, Ran, Runner},
};
use std::{os::unix::fs::MetadataExt, path::Path, time::Instant};

pub struct InstalledRunner {
    pub deadline: Instant,
}
pub fn command_for(
    root: &str,
    args: &[&str],
    env: impl IntoIterator<Item = String>,
) -> Result<std::process::Command, String> {
    let path = Path::new(root).join(".venv/bin/inbox");
    if !Path::new(root).is_absolute() {
        return Err("plugin root must be absolute".into());
    }
    let m = std::fs::symlink_metadata(&path).map_err(|_| "installed inbox unavailable")?;
    if !m.is_file() || m.nlink() != 1 || m.mode() & 0o111 == 0 {
        return Err("installed inbox must be a regular executable".into());
    }
    let navigation = match args {
        ["open"] | ["open", "--help"] | ["open", "--no-hold"] => true,
        ["open", "--question", q] | ["open", "--question", q, "--no-hold"] => {
            q.strip_prefix('Q').is_some_and(|q| {
                !q.starts_with('0')
                    && q.bytes().all(|b| b.is_ascii_digit())
                    && q.parse::<i64>()
                        .is_ok_and(|n| (1..=9_007_199_254_740_991).contains(&n))
            })
        }
        _ => false,
    };
    if !navigation {
        return Err("navigation arguments only".into());
    }
    let mut command = std::process::Command::new(path);
    command.args(args).current_dir(root);
    for k in env {
        if k.starts_with("HC_")
            || k.starts_with("UV_")
            || k.starts_with("VIRTUAL_ENV")
            || k.starts_with("PYTHON")
            || k.starts_with("INBOX_")
            || (k.starts_with("HERDR_") && k != "HERDR_SOCKET_PATH")
        {
            command.env_remove(k);
        }
    }
    command
        .env("HERDR_PLUGIN_ID", INBOX_ID)
        .env("HERDR_PLUGIN_ROOT", root)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    Ok(command)
}
impl Runner for InstalledRunner {
    async fn inbox(&self, root: &str, args: &[&str]) -> Ran {
        let env = std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned());
        let command = match command_for(root, args, env) {
            Ok(c) => c,
            Err(e) => return Ran { code: 127, out: e },
        };
        let Some(remaining) = self.deadline.checked_duration_since(Instant::now()) else {
            return Ran {
                code: 124,
                out: "navigation timed out".into(),
            };
        };
        // A single owned helper; cancellation never targets a recorded agent/server PID.
        match tokio::task::spawn_blocking(move || command::read(command, remaining, 65536, None))
            .await
        {
            Ok(Ok(o)) => Ran {
                code: o.status.code().unwrap_or(1),
                out: format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ),
            },
            _ => Ran {
                code: 124,
                out: "installed navigation helper failed or timed out".into(),
            },
        }
    }
}
