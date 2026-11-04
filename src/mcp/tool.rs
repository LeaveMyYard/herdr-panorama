//! The `report_thread` tool as the Python server lists it (`mcp_server.DESCRIPTION` and the
//! schemas pydantic made of its signature), captured in `tests/fixtures/mcp/legacy.jsonl`.
//! Deprecated: threads were removed, so the tool records nothing. It stays listed with its old
//! schema so a session told to call it before never sees "Unknown tool" (removed in N+1).

use rmcp::model::{JsonObject, Tool};
use serde_json::{Value, json};
use std::sync::Arc;

/// The tool's name.
pub const NAME: &str = "report_thread";

/// What the tool is for, as agents read it.
pub const DESCRIPTION: &str = "Deprecated: has no effect. herdr-panorama no longer has threads: it shows every\nagent and subagent under the name its spawner gave it, so there is nothing to report.\nCalls are accepted and ignored.";

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
