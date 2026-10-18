//! Reading a pane's foreground (Python `herdr.runs_panorama`, `herdr.pane_shell`,
//! `herdr.shell_state`): pure decisions over herdr's `pane.process_info`, plus the `ps` and
//! `pgrep` looks at a local shell.

use super::ops::Shell;
use herdr_agent_kit::herdr_api::ProcessInfo;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The shells herdr's session restore leaves in a pane.
pub const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "mksh", "tcsh", "csh", "nu", "xonsh", "elvish",
];

/// How long `ps` and `pgrep` may take (Python `herdr.TIMEOUT`).
pub const PS_TIMEOUT: Duration = Duration::from_secs(3);

fn base(word: &str) -> &str {
    Path::new(word)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(word)
}

/// Whether some foreground argv holds `panorama` (any path to it) followed by `command`;
/// `None` when herdr gave no list (mid-exec or mid-restore: unknown).
pub fn runs_panorama(info: &ProcessInfo, command: &str) -> Option<bool> {
    let procs = info
        .foreground_processes
        .as_ref()
        .filter(|p| !p.is_empty())?;
    Some(procs.iter().any(|p| {
        p.argv
            .windows(2)
            .any(|w| base(&w[0]) == "panorama" && w[1] == command)
    }))
}

/// Whether the foreground is nothing but the pane's own plain shell: one process, the pane's
/// `shell_pid`, whose argv is a shell (a login `-` allowed) with at most `-l`, `--login`, `-i`.
/// A process `exec`-ed in the shell keeps its pid, so the argv is checked too.
pub fn plain_shell(info: &ProcessInfo) -> Shell {
    let (Some(procs), Some(shell)) = (
        info.foreground_processes.as_ref().filter(|p| !p.is_empty()),
        info.shell_pid,
    ) else {
        return Shell::Unknown;
    };
    if procs.len() != 1 || procs[0].pid != Some(shell) {
        return Shell::Busy;
    }
    let argv = &procs[0].argv;
    let plain = argv
        .first()
        .is_some_and(|a| SHELLS.contains(&base(a).trim_start_matches('-')))
        && argv[1..]
            .iter()
            .all(|a| matches!(a.as_str(), "-l" | "--login" | "-i"));
    if plain {
        Shell::Plain(shell)
    } else {
        Shell::Busy
    }
}

/// `ps -o etime` (`[[dd-]hh:]mm:ss`) in seconds.
pub fn etime_seconds(text: &str) -> Option<f64> {
    let text = text.trim();
    let (days, rest) = match text.rsplit_once('-') {
        Some((d, r)) => (d.trim().parse::<u64>().ok()?, r),
        None => (0, text),
    };
    let parts: Vec<u64> = rest
        .split(':')
        .map(|x| x.trim().parse().ok())
        .collect::<Option<_>>()?;
    let (h, m, s) = match parts[..] {
        [m, s] => (0, m, s),
        [h, m, s] => (h, m, s),
        _ => return None,
    };
    Some((((days * 24 + h) * 60 + m) * 60 + s) as f64)
}

/// Runs a look with a deadline: its exit code and stdout, `None` when it could not run.
fn look(prog: &str, args: &[&str]) -> Option<(i32, String)> {
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let end = std::time::Instant::now() + PS_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                use std::io::Read;
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return Some((status.code()?, out));
            }
            Ok(None) if std::time::Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// A local process's age in seconds (`ps -o etime=`) and whether it has a child (`pgrep
/// -P`), each `None` when unknown. Assumes herdr runs in this pid namespace.
pub fn shell_state(pid: i64) -> (Option<f64>, Option<bool>) {
    shell_state_by(pid, look)
}

/// [`shell_state`] over `look` (the program, its args → exit code and stdout).
pub fn shell_state_by(
    pid: i64,
    look: impl Fn(&str, &[&str]) -> Option<(i32, String)>,
) -> (Option<f64>, Option<bool>) {
    let pid = pid.to_string();
    let age = look("ps", &["-o", "etime=", "-p", &pid])
        .filter(|(c, _)| *c == 0)
        .and_then(|(_, o)| etime_seconds(&o));
    // pgrep: 0 = some child, 1 = none, anything else = it could not say
    let children = match look("pgrep", &["-P", &pid]) {
        Some((0, _)) => Some(true),
        Some((1, _)) => Some(false),
        _ => None,
    };
    (age, children)
}
