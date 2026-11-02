//! Default collected Codex session hierarchy: no explicit report threads required.
use panorama_core::model::{Status, Swarm};
use panorama_core::view::nav::nav;
use panorama_core::view::{Target, View, render_dock, render_graph};
use serde_json::json;

fn swarm(harness: &str) -> Swarm {
    let mut sw: Swarm = serde_json::from_value(json!({"orchestrators":[{
        "pane_id":"pane", "tab_label":"project", "harness":harness,
        "session_id":"root", "herdr_status":"working",
        "threads":[{"id":"codex:root", "name":"Session work", "source":"session", "status":"active",
            "agents":[
                {"id":"main:codex:pane", "role":"orchestrator", "description":"Root task", "status":"run", "depth":0},
                {"id":"older01-child", "role":"worker", "description":"Older child", "status":"unknown", "parent":"main:codex:pane", "depth":1, "started":10, "seq":1},
                {"id":"younger-child", "role":"worker", "description":"Younger child", "status":"unknown", "parent":"main:codex:pane", "depth":1, "started":20, "seq":2},
                {"id":"grand01-child", "role":"worker", "description":"Grandchild one", "status":"unknown", "parent":"older01-child", "depth":2, "seq":3},
                {"id":"grand02-child", "role":"worker", "description":"Grandchild two", "status":"unknown", "parent":"younger-child", "depth":2, "seq":4}
            ]
        }]
    }]})).unwrap();
    sw.docked = Some("pane".into());
    sw
}

fn view() -> View {
    View {
        orch: Some("pane".into()),
        thread: Some("codex:root".into()),
        agent: Some("older01-child".into()),
        ..View::default()
    }
}

#[test]
fn compact_codex_shows_default_session_spawners_and_unknown_leaf_glyphs_once() {
    let sw = swarm("codex");
    let screen = render_dock(&sw, &view(), 46, 32, None, "empty");
    let text = screen.text();
    assert!(text.contains("1 active · 4 agents"), "{text}");
    for label in [
        "Session work",
        "orchestrator main",
        "worker older01",
        "worker younger",
    ] {
        assert_eq!(text.matches(label).count(), 1, "{text}");
    }
    let lines: Vec<_> = text.lines().collect();
    let root = lines
        .iter()
        .position(|l| l.contains("orchestrator main"))
        .unwrap();
    let older = lines
        .iter()
        .position(|l| l.contains("worker older01"))
        .unwrap();
    let younger = lines
        .iter()
        .position(|l| l.contains("worker younger"))
        .unwrap();
    assert!(root < older && older < younger, "{text}");
    for row in [older, younger] {
        assert!(
            lines[row].contains('?'),
            "unknown leaf must remain a live ? glyph: {text}"
        );
        assert!(lines[row].contains("└") || lines[row].contains("├"));
        assert!(!lines[row].contains('✓'), "{text}");
    }
    assert!(
        lines[root].contains("? ?"),
        "root has two direct children: {text}"
    );
    // Claude's synthetic Session placeholder still never enters the compact thread list.
    let mut changed = sw.clone();
    changed.orchestrators[0].threads[0].agents[2].status = Status::Blocked;
    let changed = render_dock(&changed, &view(), 46, 32, None, "empty").text();
    assert!(
        changed.find("worker older01").unwrap() < changed.find("worker younger").unwrap(),
        "{changed}"
    );
    let claude = render_dock(&swarm("claude"), &view(), 46, 32, None, "empty").text();
    assert!(
        !claude.contains("Session work") && !claude.contains("worker older01"),
        "{claude}"
    );
}

#[test]
fn narrow_and_wide_graphs_retain_full_default_hierarchy_and_identity() {
    let sw = swarm("codex");
    let v = view();
    let tree = nav(&sw, &v);
    assert_eq!(tree[1], ["codex:root"]);
    for width in [90, 240] {
        let mut v = v.clone();
        v.focus = 3;
        let screen = render_graph(&sw, &v, width, 60, None, "empty");
        let text = screen.text();
        assert_eq!(text.matches("Session work").count(), 1, "{width}: {text}");
        for id in ["main:codex:pane", "older01-child", "grand01-child"] {
            assert!(screen.targets.iter().any(|t| matches!(&t.target, Target::Agent(a) if a.agent == id && a.pane == "pane" && a.thread == "codex:root")), "{width} missing {id}: {text}");
        }
        if width >= 120 {
            for label in ["orchestrator main", "worker older01", "worker grand01"] {
                assert_eq!(text.matches(label).count(), 1, "{width}: {text}");
            }
        } else {
            // The existing narrow layout has selectable pods and one detail block.
            for (id, label) in [
                ("main:codex:pane", "orchestrator main"),
                ("older01-child", "worker older01"),
                ("grand01-child", "worker grand01"),
            ] {
                v.agent = Some(id.into());
                let selected = render_graph(&sw, &v, width, 60, None, "empty").text();
                assert!(selected.contains(label), "{id}: {selected}");
            }
        }
    }
    assert!(
        sw.orchestrators[0].threads[0].agents[1..]
            .iter()
            .all(|a| matches!(a.status, Status::Unknown(_)) && a.status.is_live())
    );
}

#[test]
fn compact_done_fold_and_history_unknown_status_keep_existing_semantics() {
    let mut sw = swarm("codex");
    sw.orchestrators[0].herdr_status = panorama_core::model::HerdrStatus::Done;
    // History-only descendants still appear while session is active; root done is runtime.
    let text = render_dock(&sw, &view(), 46, 32, None, "empty").text();
    assert!(
        text.contains("worker older01") && text.contains("done"),
        "{text}"
    );
    sw.orchestrators[0].threads[0].status = panorama_core::model::ThreadStatus::Done;
    let text = render_dock(&sw, &view(), 46, 32, None, "empty").text();
    assert!(text.contains("1 done"), "{text}");
    assert!(!text.contains("worker older01"), "{text}");
}
