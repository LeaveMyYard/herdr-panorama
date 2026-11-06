//! Only display fields are retained: no tool arguments, outputs, reasoning or raw errors. A
//! spawned thread's own task name (its agent path's last segment) is a display field.
use super::{Thread, Tool, Turn, TurnStatus};
use serde_json::Value;

pub(super) fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(clean).filter(|s| !s.is_empty())
}
pub(super) fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(240).collect()
}
pub(super) fn id(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .as_str()
        .filter(|s| valid_id(s))
        .map(str::to_owned)
}
pub(super) fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn time(v: &Value, key: &str) -> Option<f64> {
    v.get(key)?.as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}
pub(super) fn thread(v: &Value, expected: &str) -> Option<Thread> {
    let thread_id = id(v, "id")?;
    if thread_id != expected {
        return None;
    }
    let parent_thread_id = match v.get("parentThreadId") {
        Some(Value::Null) => None,
        Some(Value::String(parent)) if valid_id(parent) => Some(parent.clone()),
        // Missing or malformed ancestry cannot assert that this is a root.
        _ => return None,
    };
    Some(Thread {
        thread_id,
        session_tree_id: id(v, "sessionId").unwrap_or_default(),
        parent_thread_id,
        forked_from_id: id(v, "forkedFromId"),
        name: text(v, "name"),
        task_name: task_name(v),
        nickname: text(v, "agentNickname"),
        preview: text(v, "preview"),
        role: text(v, "agentRole"),
        model: text(v, "model"),
        created_at: time(v, "createdAt").unwrap_or_default(),
        updated_at: time(v, "updatedAt").unwrap_or_default(),
        ..Thread::default()
    })
}
/// A spawned thread's task name: the last segment of its agent path (`/root/a/review_auth`
/// gives `review auth`). Codex sets the path only when the spawner names the task.
fn task_name(v: &Value) -> Option<String> {
    let path = v
        .get("source")?
        .get("subAgent")?
        .get("thread_spawn")?
        .get("agent_path")?
        .as_str()?;
    let (_, last) = path.trim_end_matches('/').rsplit_once('/')?;
    (last != "root").then(|| clean(&last.replace('_', " ")))
}
pub(super) fn turn(v: &Value) -> Option<Turn> {
    Some(Turn {
        id: id(v, "id")?,
        status: match v.get("status").and_then(Value::as_str) {
            Some("completed") => TurnStatus::Completed,
            Some("failed") => TurnStatus::Failed,
            Some("interrupted") => TurnStatus::Interrupted,
            Some("inProgress") => TurnStatus::InProgress,
            _ => TurnStatus::Unknown,
        },
        started_at: time(v, "startedAt"),
        completed_at: time(v, "completedAt"),
        ..Turn::default()
    })
}
/// Items arrive newest first. The caller deduplicates item IDs across pages.
pub(super) fn item(thread: &mut Thread, entry: &Value, children: &mut Vec<String>) {
    let Some(item) = entry.get("item") else {
        return;
    };
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
    child_ids(item, children);
    let turn_id = entry.get("turnId").and_then(Value::as_str).unwrap_or("");
    let tool_name = match kind {
        "commandExecution" => Some("command".to_owned()),
        "fileChange" => Some("file change".to_owned()),
        "webSearch" => Some("web search".to_owned()),
        "imageView" => Some("view image".to_owned()),
        "imageGeneration" => Some("generate image".to_owned()),
        "mcpToolCall" | "dynamicToolCall" | "collabAgentToolCall" => text(item, "tool"),
        _ => None,
    };
    let tool = tool_name.map(|name| Tool {
        name,
        turn_id: turn_id.to_owned(),
        started_at: time(entry, "startedAtMs")
            .or_else(|| time(item, "startedAtMs"))
            .map(|n| n / 1000.0),
        completed_at: time(entry, "completedAtMs")
            .or_else(|| time(item, "completedAtMs"))
            .map(|n| n / 1000.0),
    });
    if thread.latest_tool.is_none() {
        thread.latest_tool = tool.clone();
    }
    let Some(turn) = thread.turns.iter_mut().find(|t| t.id == turn_id) else {
        return;
    };
    if let Some(tool) = tool {
        turn.tool_count += 1;
        if turn.latest_tool.is_none() {
            turn.latest_tool = Some(tool);
        }
    }
    if kind == "agentMessage" && turn.summary.is_empty() {
        turn.summary = text(item, "text").unwrap_or_default();
    }
    if kind == "userMessage" && turn.prompt.is_empty() {
        turn.prompt = item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|v| v.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|v| v.get("text").and_then(Value::as_str))
            .next()
            .map(clean)
            .unwrap_or_default();
    }
}

/// Typed references only. Metadata still has to establish the exact parent.
pub(super) fn child_ids(item: &Value, children: &mut Vec<String>) {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "subAgentActivity" {
        if let Some(child) = id(item, "agentThreadId") {
            children.push(child);
        }
    } else if kind == "collabAgentToolCall" {
        // Receiver IDs are only candidates; child metadata must prove parentage.
        for child in item
            .get("receiverThreadIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(child) = child.as_str().filter(|s| valid_id(s)) {
                children.push(child.into());
            }
        }
    }
}
