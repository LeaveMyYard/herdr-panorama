//! The "n finished" card, drawn on its own (its identity, its five pods and its names).

use super::column::Place;
use super::fin_card::fin_card;
use crate::model::Root;
use crate::view::nav::tree_entries;
use crate::view::screen::Screen;
use crate::view::state::View;
use crate::view::target::Target;
use serde_json::{Value, json};

fn root(agents: Value, hidden: i64) -> Root {
    serde_json::from_value(json!({
        "pane_id": "p", "tab_id": null, "tab_label": "p", "session_id": null,
        "hidden_finished": hidden,
        "agents": agents,
    }))
    .unwrap()
}

const PLACE: Place = Place {
    x: 0,
    y: 0,
    w: 44,
    is_sel: false,
    focus: false,
};

#[test]
fn a_finished_card_shows_five_pods_and_the_roots_own_group_is_the_roots() {
    let mut agents: Vec<_> = (0..6)
        .map(|i| {
            let title = if i % 2 == 1 { "Explore" } else { "Write" };
            json!({"id": format!("d{i}"), "title": title, "status": "done", "started": 1.8e9 - f64::from(i)})
        })
        .collect();
    agents.push(json!({"id": "live", "status": "run"}));
    let o = root(json!(agents), 0);
    let entries = tree_entries(&o, &View::default());
    let e = entries.iter().find(|e| !e.fin.is_empty()).unwrap();
    let mut cv = Screen::new(44, 3);
    fin_card(&mut cv, PLACE, e, &o, &View::default());
    let bar = "─".repeat(42);
    let want = format!("╭{bar}╮\n│  ✓   ✓   ✓   ✓   ✓   6 finished · Exp… ▸ │\n╰{bar}╯");
    assert_eq!(cv.text(), want);
    let fin = Target::Fin {
        pane: "p".into(),
        group: None,
    };
    assert_eq!(
        (cv.targets[0].x0, cv.targets[0].x1, &cv.targets[0].target),
        (0, 44, &fin)
    );
}

#[test]
fn the_roots_own_finished_card_counts_the_older_ones_the_window_hid() {
    let o = root(
        json!([{"id": "a", "title": "Fix CI", "status": "done"},
               {"id": "b", "title": "Fix CI", "status": "done"}]),
        3,
    );
    let entries = tree_entries(&o, &View::default());
    let mut cv = Screen::new(60, 3);
    let place = Place { w: 60, ..PLACE };
    fin_card(&mut cv, place, &entries[0], &o, &View::default());
    let row = cv.text().lines().nth(1).unwrap().to_owned();
    assert!(row.contains("2 finished · +3 older · Fix CI ×2"), "{row:?}");
}

#[test]
fn a_nested_groups_finished_card_names_its_group() {
    let o = root(
        json!([{"id": "a", "status": "run"}, {"id": "b", "parent": "a", "status": "done"}]),
        0,
    );
    let entries = tree_entries(&o, &View::default());
    let e = entries.iter().find(|e| !e.fin.is_empty()).unwrap();
    let mut cv = Screen::new(44, 3);
    fin_card(&mut cv, PLACE, e, &o, &View::default());
    let want = Target::Fin {
        pane: "p".into(),
        group: Some("a".into()),
    };
    assert_eq!(cv.targets[0].target, want);
}
