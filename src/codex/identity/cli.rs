//! Opt-in lifecycle hook and explicit pane binding commands.
use super::{bind, owner, store::Store};
use crate::codex::Options;
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses, parent_pid},
    herdr_api::Herdr,
    plugin_env::PluginEnv,
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    time::Duration,
};

const MAX_INPUT: usize = 64 * 1024;

fn execute_core(
    env: &PluginEnv,
    terminal: &str,
    id: &str,
    automatic: bool,
    explicit: bool,
) -> Result<(), &'static str> {
    let options = Options::from_env(env).ok_or("CODEX_HOME or HOME is required")?;
    let table = || -> Box<dyn ProcessTable> { Box::new(SystemProcesses::new()) };
    let owner = if explicit {
        None
    } else {
        Some(
            owner::ancestor(table().as_ref(), parent_pid())
                .ok_or("no owned embedded Codex process")?,
        )
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot start identity runtime")?;
    runtime.block_on(super::core::bind(
        &crate::host::Core::default(),
        env,
        &options,
        terminal,
        bind::Request {
            pane: &crate::host::context::pane_key(terminal),
            id,
            automatic,
            owner,
        },
        &table,
    ))
}

pub(super) fn candidate(value: &Value) -> Option<(&str, &str)> {
    if value.get("hook_event_name")?.as_str()? != "SessionStart"
        || value.get("agent_id").is_some()
        || value.get("parent_session_id").is_some_and(|v| !v.is_null())
    {
        return None;
    }
    let source = value.get("source")?.as_str()?;
    if !matches!(source, "startup" | "resume" | "fork") {
        return None;
    }
    let id = value.get("session_id")?.as_str()?;
    super::valid_thread_id(id).then_some((id, source))
}

fn execute(
    env: &PluginEnv,
    pane: &str,
    id: &str,
    source: &str,
    explicit: bool,
) -> Result<(), &'static str> {
    if pane.is_empty() || pane.len() > 128 {
        return Err("invalid pane ID");
    }
    let store = Store::lock(env, pane)?;
    let options = Options::from_env(env).ok_or("CODEX_HOME or HOME is required")?;
    let table = || -> Box<dyn ProcessTable> { Box::new(SystemProcesses::new()) };
    let owner =
        if explicit {
            None
        } else {
            Some(owner::ancestor(table().as_ref(), parent_pid()).ok_or(
                "no owned embedded Codex process; use codex-bind THREAD_ID --pane PANE_ID",
            )?)
        };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot start identity runtime")?;
    rt.block_on(bind::bind(
        &Herdr::from_env().with_client("panorama"),
        &options,
        bind::Request {
            pane,
            id,
            automatic: source != "manual",
            owner,
        },
        &store,
        &table,
    ))
}

/// Always a benign JSON object and success, including malformed input and telemetry failure.
/// The reader has both a byte limit and a deadline; process exit closes a stalled stdin thread.
pub fn hook() -> i32 {
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let read = std::io::stdin()
                .take((MAX_INPUT + 1) as u64)
                .read_to_end(&mut bytes);
            let _ = tx.send(
                read.ok()
                    .filter(|_| bytes.len() <= MAX_INPUT)
                    .map(|_| bytes),
            );
        });
        let bytes = rx.recv_timeout(Duration::from_secs(1)).ok().flatten()?;
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        let (id, source) = candidate(&value)?;
        let env = PluginEnv::from_env();
        if let Some(terminal) = env.var("HC_TERMINAL_ID") {
            let _ = execute_core(&env, terminal, id, true, false);
        } else {
            let pane = env.pane_id()?;
            let _ = execute(&env, pane, id, source, false);
        }
        Some(())
    });
    let _ = std::io::stdout().write_all(b"{}\n");
    0
}

pub(super) fn arguments(args: &[String]) -> Result<(&str, Option<&str>), &'static str> {
    match args {
        [id] if super::valid_thread_id(id) => Ok((id, None)),
        [id, flag, pane] if super::valid_thread_id(id) && flag == "--pane" && !pane.is_empty() => {
            Ok((id, Some(pane)))
        }
        _ => Err(
            "usage: panorama codex-bind THREAD_ID [--pane PANE_ID] (canonical lower-case UUID required)",
        ),
    }
}

/// An explicitly chosen pane permits binding existing shared-daemon sessions.
pub fn manual(args: &[String]) -> i32 {
    let env = PluginEnv::from_env();
    let result = if let [id, flag, terminal] = args
        && flag == "--terminal"
    {
        execute_core(&env, terminal, id, false, true)
    } else {
        arguments(args).and_then(|(id, pane)| {
            if pane.is_none()
                && let Some(terminal) = env.var("HC_TERMINAL_ID")
            {
                return execute_core(&env, terminal, id, false, false);
            }
            let target = pane
                .or_else(|| env.pane_id())
                .ok_or("pass --pane PANE_ID outside an embedded Codex pane")?;
            execute(&env, target, id, "manual", pane.is_some())
        })
    };
    match result {
        Ok(()) => 0,
        Err(why) => {
            eprintln!("panorama codex-bind: {why}");
            1
        }
    }
}
