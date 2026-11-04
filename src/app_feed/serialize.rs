//! Feed v2 (`docs/protocol/app-feed-v2.md`): the swarm as `snapshot --json` prints it, roots
//! with one flat agent tree each, adding only the app's provenance (`v`, root `host` and
//! `terminal`, agent `harness`).
use super::World;
use crate::{
    host::Host,
    snapshot::{json, pyjson::J},
};
use serde_json::{Map, Number, Value};
use std::collections::HashSet;

/// The feed version this writer emits and publishes (`docs/protocol/app-feed-v2.md`).
pub const VERSION: i64 = 2;
const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

fn value(j: J) -> Result<Value, String> {
    Ok(match j {
        J::Null => Value::Null,
        J::Bool(v) => Value::Bool(v),
        J::Str(v) => Value::String(v),
        J::Int(v) if (-SAFE_INTEGER..=SAFE_INTEGER).contains(&v) => Value::Number(v.into()),
        J::Int(_) => return Err("feed integer exceeds the safe range".into()),
        J::Float(v) => {
            Value::Number(Number::from_f64(v).ok_or("feed contains a nonfinite timestamp")?)
        }
        J::Arr(items) => Value::Array(items.into_iter().map(value).collect::<Result<_, _>>()?),
        J::Obj(items) => Value::Object(
            items
                .into_iter()
                .map(|(k, v)| Ok((k, value(v)?)))
                .collect::<Result<Map<_, _>, String>>()?,
        ),
    })
}

fn harness(s: &str) -> &str {
    if s.is_empty() { "unknown" } else { s }
}

fn validate(world: &World) -> Result<(), String> {
    if world.roots.len() != world.swarm.roots.len() {
        return Err("root provenance does not match the swarm".into());
    }
    let mut roots = HashSet::new();
    for (root, meta) in world.swarm.roots.iter().zip(&world.roots) {
        let key = if meta.host == Host::Core {
            meta.terminal.as_deref().unwrap_or(&root.pane_id)
        } else {
            &root.pane_id
        };
        if root.pane_id.is_empty() || !roots.insert((meta.host.code(), key)) {
            return Err("invalid feed root identity or count".into());
        }
        if meta
            .terminal
            .as_deref()
            .is_some_and(|t| !crate::host::context::valid_address(t))
        {
            return Err("invalid feed terminal address".into());
        }
        if root.inbox.len() != root.inbox_texts.len()
            || root.inbox.iter().collect::<HashSet<_>>().len() != root.inbox.len()
        {
            return Err("invalid root question inventory".into());
        }
        if !world.swarm.inbox_ok && !root.inbox.is_empty() {
            return Err("unavailable inbox has questions".into());
        }
        if root.hidden_finished < 0 {
            return Err("negative hidden_finished".into());
        }
        let mut agents = HashSet::new();
        for a in &root.agents {
            if a.id.is_empty() || !agents.insert(&a.id) || a.tool_calls < 0 || a.resumes < 0 {
                return Err("invalid feed agent identity or count".into());
            }
            if a.question_ids.len() != a.questions.len()
                || a.question_ids.iter().collect::<HashSet<_>>().len() != a.question_ids.len()
            {
                return Err("invalid agent question inventory".into());
            }
            for (id, text) in a.question_ids.iter().zip(&a.questions) {
                if root
                    .inbox
                    .iter()
                    .position(|q| q == id)
                    .is_none_or(|i| root.inbox_texts[i] != *text)
                {
                    return Err("invalid agent question attribution".into());
                }
            }
        }
    }
    Ok(())
}

pub fn model(world: &World) -> Result<Value, String> {
    validate(world)?;
    let mut feed = value(json::swarm(&world.swarm))?;
    feed["v"] = Value::Number(VERSION.into());
    for ((row, root), meta) in feed["roots"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&world.swarm.roots)
        .zip(&world.roots)
    {
        row["host"] = Value::String(meta.host.code().into());
        row["terminal"] = meta.terminal.clone().map_or(Value::Null, Value::String);
        row["harness"] = Value::String(harness(&root.harness).into());
        if meta.host == Host::Core {
            row["tab_id"] = Value::Null;
            row["workspace_id"] = Value::Null;
            row["tab_no"] = Value::Null;
            row["focused"] = Value::Bool(false);
        }
        for agent in row["agents"].as_array_mut().unwrap() {
            let supplied = agent["id"]
                .as_str()
                .and_then(|id| meta.agent_harnesses.get(id));
            agent["harness"] =
                Value::String(harness(supplied.map_or("unknown", String::as_str)).into());
        }
    }
    Ok(feed)
}

pub fn bytes(world: &World) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(&model(world)?).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}
