//! `panorama hook <kind>`: the opt-in Claude Code hooks (Python `hooks.py`), fast, silent and
//! never failing. `subagent-start` and `subagent-stop` append one record per event to the
//! session's hook file, which the scanner reads; `session-start` records the session's
//! process (a resume shows up as a new one), prunes hook files older than 14 days and, the
//! first time a session starts inside herdr, prints the one line about `report_thread`.
//!
//! Exit status 0 always: for SubagentStop, exit 2 would keep the subagent running and feed its
//! stderr back to the agent, so nothing on this path may ever exit 2. Bad input, an unwritable
//! dir, a panic: all exit 0 with nothing on stdout.
//!
//! Stated deviations (payloads Claude Code does not send): JSON that only Python's `json.loads`
//! takes (`NaN`, `Infinity`, `1E400`) is no payload here, so nothing is
//! recorded or said; a lone surrogate escape reads as U+FFFD (Python keeps the surrogate and
//! writes it back escaped); an integer past 64 bits in `source` is written as a float, and an object
//! `source` with its keys sorted.

pub mod record;
pub mod session;

use herdr_agent_kit::agent_procs::claude::claude_process_here;
use herdr_agent_kit::agent_procs::own_pidns;
use herdr_agent_kit::agent_session::claude::wire::Line;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::{Map, Value};
use std::io::{Read, Write};
use std::path::PathBuf;

/// The Claude Code process above the hook: (pid, start token, pid namespace).
pub type Identity = Option<(u32, String, Option<String>)>;

/// What a hook reads besides its payload.
pub struct Ctx {
    /// Where hook files live.
    pub hooks_dir: PathBuf,
    /// Inside herdr (`HERDR_PANE_ID` or `HERDR_ENV`).
    pub in_herdr: bool,
    /// Now (POSIX seconds).
    pub now: f64,
    /// The Claude process above this one, if it is recognised.
    pub identity: Box<dyn Fn() -> Identity>,
}

impl Ctx {
    /// From the environment and this process's ancestry.
    pub fn from_env(env: &PluginEnv) -> Self {
        Self {
            hooks_dir: crate::collect::paths::hooks_dir(env),
            in_herdr: env.in_herdr(),
            now: crate::snapshot::now(),
            identity: Box::new(|| {
                claude_process_here().map(|(pid, start)| (pid, start, own_pidns()))
            }),
        }
    }
}

/// The payload as Python's `json.loads` reads it: an object, else nothing. A lone UTF-16
/// surrogate escape (Node writes one when it cuts a string inside an emoji) is read through
/// the kit's repair, as U+FFFD; a leading BOM is refused, as `json.loads` refuses it.
pub fn payload(raw: &str) -> Map<String, Value> {
    if raw.starts_with('\u{feff}') {
        return Map::new();
    }
    Line::parse(raw.as_bytes())
        .map(Line::into_map)
        .unwrap_or_default()
}

/// One hook run: the payload on `input`, the output (if any) on `output`. Always 0.
pub fn run(kind: &str, input: &mut dyn Read, output: &mut dyn Write, cx: &Ctx) -> i32 {
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut raw = String::new();
        input.read_to_string(&mut raw).ok()?;
        let payload = payload(&raw);
        match kind {
            "subagent-start" => record::record("start", &payload, cx),
            "subagent-stop" => record::record("stop", &payload, cx),
            "session-start" => return session::session_start(&payload, cx),
            _ => {}
        }
        None
    }));
    if let Ok(Some(text)) = out {
        let _ = output.write_all(text.as_bytes());
        let _ = output.flush();
    }
    0
}

/// `panorama hook <kind>` on stdin and stdout; the exit status (0).
pub fn main(kind: &str) -> i32 {
    // a panic's message goes nowhere: the hook stays silent
    std::panic::set_hook(Box::new(|_| {}));
    let cx = Ctx::from_env(&PluginEnv::from_env());
    run(kind, &mut std::io::stdin(), &mut std::io::stdout(), &cx)
}

#[cfg(test)]
mod tests;
