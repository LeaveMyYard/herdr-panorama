//! The `report_thread` tool as the Python server lists it (`mcp_server.DESCRIPTION` and the
//! schemas pydantic made of its signature), captured in `tests/fixtures/mcp/legacy.jsonl`:
//! agents read this text, so it stays the same word for word.

use rmcp::model::{JsonObject, Tool};
use serde_json::{Value, json};
use std::sync::Arc;

/// The tool's name.
pub const NAME: &str = "report_thread";

/// What the tool is for, as agents read it.
pub const DESCRIPTION: &str = "Declare or update a thread of work for the herdr swarm view (herdr-panorama).\n\nCall it when multi-step work starts (a PR through review rounds, a plan's tasks, several\nsubagents on one goal) and whenever its phase changes. Returns immediately.\n- thread_id: stable id, e.g. \"pr-595\" or \"task-3\"; title: short name, e.g. \"PR #595 ledger\".\n- phase: the current phase (\"review r2\"); phases_done: finished phases in order;\n  next: the phases still to come. Omitted lists keep what you reported before.\n- status: \"active\" (default), \"done\", \"failed\" or \"cancelled\" when the thread ends.\n- agents: optional subagent ids or descriptions (a trailing * matches a prefix) that work on it.";

fn object(v: Value) -> Arc<JsonObject> {
    match v {
        Value::Object(m) => Arc::new(m),
        _ => Arc::default(),
    }
}

/// `report_thread`'s arguments, as pydantic described them.
pub fn input_schema() -> Value {
    json!({
        "properties": {
            "thread_id": {
                "title": "Thread Id",
                "type": "string"
            },
            "title": {
                "anyOf": [
                    {
                        "type": "string"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Title"
            },
            "phase": {
                "anyOf": [
                    {
                        "type": "string"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Phase"
            },
            "phases_done": {
                "anyOf": [
                    {
                        "items": {
                            "type": "string"
                        },
                        "type": "array"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Phases Done"
            },
            "next": {
                "anyOf": [
                    {
                        "items": {
                            "type": "string"
                        },
                        "type": "array"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Next"
            },
            "status": {
                "anyOf": [
                    {
                        "type": "string"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Status"
            },
            "agents": {
                "anyOf": [
                    {
                        "items": {
                            "type": "string"
                        },
                        "type": "array"
                    },
                    {
                        "type": "null"
                    }
                ],
                "default": null,
                "title": "Agents"
            }
        },
        "required": [
            "thread_id"
        ],
        "type": "object",
        "title": "report_threadArguments"
    })
}

/// Its result: any object.
pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "title": "report_threadDictOutput"
    })
}

/// The tool, as `tools/list` lists it.
pub fn tool() -> Tool {
    let mut t = Tool::new(NAME, DESCRIPTION, object(input_schema()));
    t.output_schema = Some(object(output_schema()));
    t
}

#[cfg(test)]
mod tests;
