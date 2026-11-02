use super::*;
use crate::codex::{Thread, Tool, Turn, TurnStatus};
use crate::collect::events::Channel;
use panorama_core::model::{HerdrStatus, PhaseState, Status, ThreadStatus};
use serde_json::json;

mod history;
mod identity;
mod reports;

fn pane(id: &str) -> Orchestrator {
    serde_json::from_value(json!({
        "pane_id":"w1:p1", "tab_id":"w1:t1", "tab_label":"Synthetic work",
        "session_id":id, "harness":"codex", "herdr_status":"working"
    }))
    .unwrap()
}

fn thread(id: &str, parent: Option<&str>) -> Thread {
    Thread {
        thread_id: id.into(),
        session_tree_id: "shared-session-group".into(),
        parent_thread_id: parent.map(str::to_owned),
        created_at: 10.0,
        updated_at: 30.0,
        name: Some(format!("Synthetic {id}")),
        model: Some("synthetic-model".into()),
        turns: vec![Turn {
            id: format!("{id}-turn"),
            status: TurnStatus::Completed,
            started_at: Some(20.0),
            completed_at: Some(30.0),
            prompt: "Synthetic task".into(),
            summary: "Synthetic result".into(),
            tool_count: 2,
            latest_tool: Some(Tool {
                name: "synthetic_tool".into(),
                turn_id: format!("{id}-turn"),
                ..Tool::default()
            }),
        }],
        ..Thread::default()
    }
}

fn observed() -> Observation {
    Observation {
        root: thread("root", None),
        children: vec![
            thread("child", Some("root")),
            thread("grandchild", Some("child")),
        ],
        partial: false,
        observed_at: 35.0,
    }
}

fn channel() -> Channel {
    Channel::new(std::path::PathBuf::from("synthetic-unused-events.jsonl"))
}

fn apply(pane: &mut Orchestrator, observation: Option<&Observation>, channel: &Channel) {
    let reports = [channels::Reports {
        source: pane.session_id.clone().unwrap(),
        declarations: channel.threads.clone(),
    }];
    enrich(pane, observation, &reports, 40.0, 3600.0);
}
