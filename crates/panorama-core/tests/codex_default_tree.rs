//! Default collected Codex session hierarchy: no explicit report threads required. Agents
//! carry hand-written titles of the kinds the collector gives (plan `uniform-agent-tree.md`
//! §2.2, U1b): the spawner's task name, Codex's nickname, the first prompt; the root's own
//! card has none. The tree has the uniform shape (U3): one thread of all the root's agents,
//! the root's own children with no parent, no synthetic main agent.
use panorama_core::model::{Status, Swarm};
use panorama_core::view::nav::nav;
use panorama_core::view::{Target, View, render_dock, render_graph};
use serde_json::json;

fn swarm(harness: &str) -> Swarm {
    let mut sw: Swarm = serde_json::from_value(json!({"roots":[{
        "pane_id":"pane", "tab_label":"project", "title":"project", "title_source":"tab", "harness":harness,
        "session_id":"root", "herdr_status":"working",
        "agents": [{"id":"older01-child", "role":"worker", "description":"Older child", "title":"review auth", "title_source":"spawner", "status":"unknown", "depth":1, "started":10, "seq":1},
                {"id":"younger-child", "role":"worker", "description":"Younger child", "title":"Euclid", "title_source":"session", "status":"unknown", "depth":1, "started":20, "seq":2},
                {"id":"grand01-child", "role":"worker", "description":"Grandchild one", "title":"Fix the flaky test", "title_source":"prompt", "status":"unknown", "parent":"older01-child", "depth":2, "seq":3},
                {"id":"grand02-child", "role":"worker", "description":"Grandchild two", "title":"worker grand02", "title_source":"type", "status":"unknown", "parent":"younger-child", "depth":2, "seq":4}]
    }]})).unwrap();
    sw.docked = Some("pane".into());
    sw
}

fn view() -> View {
    View {
        orch: Some("pane".into()),
        agent: Some("older01-child".into()),
        ..View::default()
    }
}

#[test]
fn compact_codex_shows_two_layers_of_the_roots_agents_and_unknown_leaf_glyphs_once() {
    let sw = swarm("codex");
    let screen = render_dock(&sw, &view(), 46, 32, None, "empty");
    let text = screen.text();
    assert!(text.contains("2 active · 4 agents"), "{text}");
    assert!(!text.contains("Session work"), "no thread row: {text}");
    let lines: Vec<_> = text.lines().collect();
    let row = |label: &str| {
        assert_eq!(text.matches(label).count(), 1, "{label}: {text}");
        lines.iter().position(|l| l.contains(label)).unwrap()
    };
    let (older, grand1) = (row("review auth"), row("Fix the flaky test"));
    let (younger, grand2) = (row("Euclid"), row("worker grand02"));
    assert!(
        older < grand1 && grand1 < younger && younger < grand2,
        "{text}"
    );
    for r in [older, grand1, younger, grand2] {
        assert!(
            lines[r].contains('?'),
            "unknown leaf must remain a live ? glyph: {text}"
        );
        assert!(!lines[r].contains('✓'), "{text}");
    }
    // layer 1 hangs off the entry's branch, layer 2 off its parent's
    assert!(
        lines[older].contains("├ ?") && lines[younger].contains("└ ?"),
        "{text}"
    );
    assert!(
        lines[grand1].contains("│ └ ?") && lines[grand2].contains("  └ ?"),
        "{text}"
    );
    // a status change never reorders rows
    let mut changed = sw.clone();
    changed.roots[0].agents[1].status = Status::Blocked;
    let changed = render_dock(&changed, &view(), 46, 32, None, "empty").text();
    assert!(
        changed.find("review auth").unwrap() < changed.find("Euclid").unwrap(),
        "{changed}"
    );
    // one tree for every harness: Claude's rows are Codex's
    let claude = render_dock(&swarm("claude"), &view(), 46, 32, None, "empty").text();
    for label in [
        "review auth",
        "Euclid",
        "Fix the flaky test",
        "worker grand02",
    ] {
        assert_eq!(claude.matches(label).count(), 1, "{claude}");
    }
}

#[test]
fn narrow_and_wide_graphs_retain_full_default_hierarchy_and_identity() {
    let sw = swarm("codex");
    let v = view();
    let tree = nav(&sw, &v);
    assert_eq!(tree[1], ["older01-child", "younger-child"]);
    for width in [90, 240] {
        let mut v = v.clone();
        v.focus = 2;
        let screen = render_graph(&sw, &v, width, 60, None, "empty");
        let text = screen.text();
        assert!(
            !text.contains("Session work"),
            "no thread card: {width}: {text}"
        );
        for id in ["older01-child", "grand01-child"] {
            assert!(
                screen.targets.iter().any(
                    |t| matches!(&t.target, Target::Agent(a) if a.agent == id && a.pane == "pane")
                ),
                "{width} missing {id}: {text}"
            );
        }
        if width >= 120 {
            for label in ["review auth", "Fix the flaky test"] {
                assert_eq!(text.matches(label).count(), 1, "{width}: {text}");
            }
            // The selected titled card keeps its role and id on the secondary line.
            assert!(text.contains("worker older01"), "{width}: {text}");
        } else {
            // The existing narrow layout has selectable pods and one detail block.
            for (id, label) in [
                ("older01-child", "review auth"),
                ("grand01-child", "Fix the flaky test"),
            ] {
                v.agent = Some(id.into());
                let selected = render_graph(&sw, &v, width, 60, None, "empty").text();
                assert!(selected.contains(label), "{id}: {selected}");
            }
        }
    }
    assert!(
        sw.roots[0]
            .agents
            .iter()
            .all(|a| matches!(a.status, Status::Unknown(_)) && a.status.is_live())
    );
}

#[test]
fn compact_done_fold_and_history_unknown_status_keep_existing_semantics() {
    let mut sw = swarm("codex");
    sw.roots[0].herdr_status = panorama_core::model::HerdrStatus::Done;
    // History-only descendants still appear while they are live; root done is runtime.
    let text = render_dock(&sw, &view(), 46, 32, None, "empty").text();
    assert!(
        text.contains("review auth") && text.contains("done"),
        "{text}"
    );
    for a in &mut sw.roots[0].agents {
        a.status = Status::Done;
    }
    sw.roots[0].hidden_finished = 3;
    let text = render_dock(&sw, &view(), 46, 32, None, "empty").text();
    // the root's finished subagents fold into one row; theirs get none
    assert!(text.contains("2 finished · review auth"), "{text}");
    assert!(text.contains("+3 older"), "{text}");
    assert!(!text.contains("Fix the flaky test"), "{text}");
}
