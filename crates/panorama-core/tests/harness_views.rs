//! Harness names occupy the existing title area; all focus targets retain pane identity.
use panorama_core::model::Swarm;
use panorama_core::view::nav::nav;
use panorama_core::view::{Filter, Target, View, render_dock, render_graph};
use serde_json::json;

fn swarm() -> Swarm {
    serde_json::from_value(json!({"orchestrators":[
        {"pane_id":"claude-pane","tab_id":"c","tab_label":"same title","session_id":null,
         "harness":"claude", "herdr_status":"working", "tab_no":1},
        {"pane_id":"codex-pane","tab_id":"x","tab_label":"same title","session_id":null,
         "harness":"codex", "herdr_status":"blocked", "tab_no":2}
    ]}))
    .unwrap()
}

#[test]
fn mixed_harnesses_are_named_and_click_targets_still_identify_exact_panes() {
    let swarm = swarm();
    let view = View {
        orch: Some("claude-pane".into()),
        ..View::default()
    };
    for (dock, width) in [(true, 46), (true, 30), (false, 70), (false, 160)] {
        let screen = if dock {
            render_dock(&swarm, &view, width, 32, None, "empty")
        } else {
            render_graph(&swarm, &view, width, 32, None, "empty")
        };
        let text = screen.text();
        assert!(text.contains("Claude Code"), "{dock}/{width}: {text}");
        assert!(text.contains("Codex"), "{dock}/{width}: {text}");
        for (name, pane) in [("Claude Code", "claude-pane"), ("Codex", "codex-pane")] {
            let (y, line) = text
                .lines()
                .enumerate()
                .find(|(_, l)| l.contains(name))
                .unwrap();
            let x = line.chars().collect::<String>().find(name).unwrap();
            // Unicode byte offsets differ: count the characters preceding the label.
            let x = line[..x].chars().count();
            assert_eq!(
                screen.target_at(x as i32, y as i32),
                Some(&Target::Orch(pane.into()))
            );
        }
    }
}

#[test]
fn mixed_navigation_and_filters_use_pane_ids() {
    let sw = swarm();
    let mut view = View::default();
    assert_eq!(
        nav(&sw, &view)[0],
        ["claude-pane", "codex-pane", panorama_core::view::NEW_ROW]
    );
    view.filter = Filter::Needs;
    assert_eq!(
        nav(&sw, &view)[0],
        ["codex-pane", panorama_core::view::NEW_ROW]
    );
}

fn codex(runtime: &str, history: Option<&str>) -> Swarm {
    let mut sw = swarm();
    sw.orchestrators.remove(0);
    let o = &mut sw.orchestrators[0];
    o.herdr_status = panorama_core::model::HerdrStatus::from_code(runtime);
    if let Some(status) = history {
        o.session_id = Some("exact-root".into());
        o.threads.push(serde_json::from_value(json!({
            "id":"codex:exact-root", "name":"History", "source":"session", "status":status,
            "agents":[
                {"id":"main:codex:codex-pane", "description":"Root", "role":"orchestrator",
                 "status":match runtime { "working"=>"run", "blocked"=>"blk", "idle"=>"q", other=>other }},
                {"id":"verified-child", "description":"main", "role":"main", "status":"unknown"}
            ]
        })).unwrap());
    }
    sw
}

fn assert_layout_state(sw: &Swarm, label: &str, done: bool) {
    let view = View {
        orch: Some("codex-pane".into()),
        ..View::default()
    };
    for (dock, width) in [(true, 46), (false, 70), (false, 160)] {
        let screen = if dock {
            render_dock(sw, &view, width, 32, None, "empty")
        } else {
            render_graph(sw, &view, width, 32, None, "empty")
        };
        let text = screen.text();
        let row = text.lines().position(|r| r.contains("Codex")).unwrap();
        let right = if !dock && width >= 120 {
            screen
                .columns
                .iter()
                .find(|column| column.column == 0)
                .unwrap()
                .x1 as usize
        } else {
            width as usize
        };
        let status_rows = text
            .lines()
            .skip(row)
            .take(if !dock && width >= 120 { 2 } else { 1 })
            .map(|line| line.chars().take(right).collect::<String>())
            .collect::<Vec<_>>()
            .join(" ");
        let expected = if dock && done { "done" } else { label };
        assert!(
            status_rows.contains(expected),
            "{dock}/{width}: {status_rows}"
        );
        assert_eq!(
            status_rows.contains("done"),
            done,
            "{dock}/{width}: {status_rows}"
        );
    }
}

#[test]
fn codex_runtime_survives_history_arrival_in_every_layout() {
    for (runtime, label) in [
        ("idle", "idle"),
        ("unknown", "unknown"),
        ("done", "all done"),
        ("working", "working"),
        ("blocked", "blocked"),
    ] {
        for history in [None, Some("active"), Some("done"), Some("unknown")] {
            assert_layout_state(&codex(runtime, history), label, runtime == "done");
        }
    }
}

#[test]
fn codex_child_and_report_attention_survives_root_state_and_main_names() {
    for runtime in ["done", "unknown", "idle"] {
        for source in ["session", "explicit"] {
            for (status, label) in [
                ("run", "working"),
                ("blk", "blocked"),
                ("wait", "needs you"),
                ("fail", "failed"),
                ("bg", "waiting"),
            ] {
                let mut sw = codex(runtime, Some("done"));
                let thread = &mut sw.orchestrators[0].threads[0];
                thread.source = panorama_core::model::ThreadSource::from_code(source);
                // A distinct child can have the root's name/role and even a main prefix.
                let child = &mut thread.agents[1];
                child.id = "main:distinct-child".into();
                child.status = panorama_core::model::Status::from_code(status);
                assert_layout_state(&sw, label, false);
            }
        }
        let mut sw = codex(runtime, Some("active"));
        sw.orchestrators[0].inbox.push(1);
        assert_layout_state(&sw, "needs you", false);
        sw.orchestrators[0].inbox.clear();
        sw.orchestrators[0].threads[0].agents[1]
            .question_ids
            .push(2);
        assert_layout_state(&sw, "needs you", false);
        sw.orchestrators[0].threads[0].agents[1]
            .question_ids
            .clear();
        sw.orchestrators[0].bg.push(
            serde_json::from_value(json!({
                "id":"task", "kind":"task", "description":"Synthetic", "started":null
            }))
            .unwrap(),
        );
        assert_layout_state(&sw, "waiting", false);
    }
}
