//! The "n finished" card, drawn on its own (its identity and its five pods).

use super::column::Place;
use super::fin_card::fin_card;
use crate::model::Thread;
use crate::view::nav::tree_entries;
use crate::view::screen::Screen;
use crate::view::state::View;
use crate::view::target::Target;
use serde_json::json;

#[test]
fn a_finished_card_shows_five_pods_and_the_threads_own_group_keeps_its_identity() {
    // captured from Python's render.fin_card on the same thread
    let mut agents: Vec<_> = (0..6)
        .map(|i| {
            let role = if i % 2 == 1 { "explorer" } else { "writer" };
            json!({"id": format!("d{i}"), "role": role, "status": "done", "started": 1.8e9 - f64::from(i)})
        })
        .collect();
    agents.push(json!({"id": "live", "status": "run"}));
    let t: Thread =
        serde_json::from_value(json!({"id": "t", "name": "n", "source": "turn", "agents": agents}))
            .unwrap();
    let view = View {
        orch: Some("p".into()),
        ..View::default()
    };
    let entries = tree_entries(&t, &View::default());
    let e = entries.iter().find(|e| !e.fin.is_empty()).unwrap();
    let mut cv = Screen::new(44, 3);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: false,
        focus: false,
    };
    fin_card(&mut cv, place, e, &t, &view);
    let bar = "─".repeat(42);
    let want = format!("╭{bar}╮\n│  ✓   ✓   ✓   ✓   ✓   6 finished · exp… ▸ │\n╰{bar}╯");
    assert_eq!(cv.text(), want);
    let fin = Target::Fin {
        pane: "p".into(),
        thread: "t".into(),
        group: None,
    };
    assert_eq!(
        (cv.targets[0].x0, cv.targets[0].x1, &cv.targets[0].target),
        (0, 44, &fin)
    );
}

#[test]
fn a_nested_groups_finished_card_names_its_group() {
    let t: Thread =
        serde_json::from_value(json!({"id": "t", "name": "n", "source": "turn", "agents": [
            {"id": "a", "status": "run"}, {"id": "b", "parent": "a", "status": "done"},
        ]}))
        .unwrap();
    let entries = tree_entries(&t, &View::default());
    let e = entries.iter().find(|e| !e.fin.is_empty()).unwrap();
    let mut cv = Screen::new(44, 3);
    let place = Place {
        x: 0,
        y: 0,
        w: 44,
        is_sel: false,
        focus: false,
    };
    fin_card(&mut cv, place, e, &t, &View::default());
    let want = Target::Fin {
        pane: String::new(),
        thread: "t".into(),
        group: Some("a".into()),
    };
    assert_eq!(cv.targets[0].target, want);
}
