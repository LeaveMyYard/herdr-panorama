//! `panorama emit`: declare or update a thread on the explicit channel by hand (Python
//! `cli.cmd_emit`): the same record a `report_thread` call writes, appended through
//! [`crate::report::emit`]. Prints the events file's path.
//!
//! Stated deviations: `--json` that is neither an object nor a list of `[key, value]` pairs is
//! refused (exit 2; Python fails with a traceback, exit 1), an object's keys are merged in
//! sorted order, and `@<phase>` in `--agent` counts ASCII digits only (Python crashes on
//! others).

pub mod args;
pub mod specs;

use crate::collect::paths::{events_dir, events_path};
use crate::snapshot::pyjson::J;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::Value;
use std::path::PathBuf;

/// Set `key` in a record: in its place when it is there, else at the end (`dict.update`).
fn set(rec: &mut Vec<(String, J)>, key: &str, value: J) {
    match rec.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = value,
        None => rec.push((key.to_owned(), value)),
    }
}

/// The record `a` declares (before `v` and `ts`), or why not.
pub fn record(a: &args::Args) -> Result<Vec<(String, J)>, String> {
    let mut rec = vec![("thread".to_owned(), J::str(&a.thread))];
    for (k, v) in [
        ("name", &a.name),
        ("flavour", &a.flavour),
        ("status", &a.status),
        ("worktree", &a.worktree),
    ] {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
            rec.push((k.to_owned(), J::str(v)));
        }
    }
    if let Some(pct) = a.pct {
        rec.push(("pct".to_owned(), J::Int(pct)));
    }
    if !a.phases.is_empty() {
        rec.push((
            "phases".to_owned(),
            J::Arr(a.phases.iter().map(|s| specs::phase(s)).collect()),
        ));
    }
    if !a.agents.is_empty() {
        rec.push((
            "agents".to_owned(),
            J::Arr(a.agents.iter().map(|s| specs::agent(s)).collect()),
        ));
    }
    // Python's `if args.json: rec.update(json.loads(args.json))`: an empty value is none, and
    // `update` takes an object or a list of `[key, value]` pairs
    if let Some(text) = a.json.as_deref().filter(|t| !t.is_empty()) {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(extra)) => {
                for (k, v) in &extra {
                    set(&mut rec, k, J::from(v));
                }
            }
            Ok(Value::Array(pairs)) => {
                for pair in &pairs {
                    match pair.as_array().map(Vec::as_slice) {
                        Some([Value::String(k), v]) => set(&mut rec, k, J::from(v)),
                        _ => return Err("--json: a JSON object".to_owned()),
                    }
                }
            }
            _ => return Err("--json: a JSON object".to_owned()),
        }
    }
    Ok(rec)
}

/// What a successful `emit` did.
#[derive(Clone, Debug, PartialEq)]
pub enum Done {
    /// The help was asked for.
    Help,
    /// Wrote to this events file.
    Wrote(PathBuf),
}

/// `panorama emit <argv>` for `env` at `now`: what it did, or (exit status, the error text).
pub fn emit(argv: &[String], env: &PluginEnv, now: f64) -> Result<Done, (i32, String)> {
    let a = match args::parse(argv).map_err(|e| (2, e))? {
        args::Parsed::Help => return Ok(Done::Help),
        args::Parsed::Emit(a) => a,
    };
    let session = a
        .session
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| env.var("CLAUDE_CODE_SESSION_ID").map(str::to_owned));
    let Some(session) = session else {
        return Err((
            2,
            "panorama emit: no session (pass --session or run inside Claude Code)".into(),
        ));
    };
    let rec = record(&a).map_err(|e| (2, format!("panorama emit: error: {e}")))?;
    let path = events_path(&events_dir(env), &session);
    crate::report::emit(&path, rec, now).map_err(|e| (1, format!("panorama emit: {e}")))?;
    Ok(Done::Wrote(path))
}

/// `panorama emit` on this process; the exit status.
pub fn run(argv: &[String]) -> i32 {
    match emit(argv, &PluginEnv::from_env(), crate::snapshot::now()) {
        Ok(Done::Wrote(path)) => {
            println!("{}", path.display());
            0
        }
        Ok(Done::Help) => {
            print!("{}", args::HELP);
            0
        }
        Err((code, why)) => {
            eprintln!("{why}");
            code
        }
    }
}

#[cfg(test)]
mod tests;
