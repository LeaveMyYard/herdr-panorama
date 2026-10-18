use super::*;
use serde_json::json;

/// The rows `test_machines.py`'s `profiles` fixture serves as `machine list --json`.
pub(crate) fn profiles() -> Vec<Saved> {
    serde_json::from_value(json!([
        {"id": "abc123", "label": "devbox", "target": "devbox", "session": "default", "enabled": true},
        {"id": "def456", "label": "lab", "target": "lab", "session": "default", "enabled": true},
        {"id": "off789", "label": "old", "target": "old", "session": "default", "enabled": false},
    ]))
    .unwrap()
}

pub(crate) struct Fixed(pub Selection, pub Vec<Saved>);

impl Source for Fixed {
    fn selection(&mut self) -> Selection {
        self.0.clone()
    }
    fn machines(&mut self) -> Vec<Saved> {
        self.1.clone()
    }
}

fn sel(v: serde_json::Value) -> Selection {
    Selection::from_config(json!({ "machines": v }).as_object().unwrap())
}

fn labels(m: &[Machine]) -> Vec<&str> {
    m.iter().map(|m| m.label.as_str()).collect()
}

#[test]
fn the_setting_reads_as_python_reads_it() {
    assert_eq!(Selection::from_config(&Map::new()), Selection::All);
    for none in [
        json!("none"),
        json!(" Off "),
        json!(false),
        json!(0),
        json!(""),
    ] {
        assert_eq!(sel(none.clone()), Selection::None, "{none}");
    }
    for all in [
        json!("all"),
        json!("anything"),
        json!(true),
        json!(1),
        json!(0.0),
    ] {
        assert_eq!(sel(all.clone()), Selection::All, "{all}");
    }
    assert_eq!(
        sel(json!(["lab", 1, true])),
        Selection::Labels(vec!["lab".into(), "1".into(), "True".into()])
    );
}

#[test]
fn saved_machines_follow_the_config() {
    let mut all = Fixed(Selection::All, profiles());
    assert_eq!(labels(&saved(&mut all)), ["devbox", "lab"]); // default: all enabled ones
    let mut some = Fixed(
        Selection::Labels(vec!["lab".into(), "old".into()]),
        profiles(),
    );
    assert_eq!(labels(&saved(&mut some)), ["lab"]); // a disabled profile stays off
    let mut none = Fixed(Selection::None, profiles());
    assert!(saved(&mut none).is_empty());
}

#[test]
fn unsafe_ssh_targets_are_never_used() {
    let rows = serde_json::from_value(json!([
        {"id": "a", "label": "ok", "target": "devbox", "enabled": true},
        {"id": "b", "label": "opt", "target": "-oProxyCommand=x", "enabled": true},
        {"id": "c", "label": "space", "target": "dev box", "enabled": true},
        {"id": "", "label": "no id", "target": "x", "enabled": true},
        {"id": "d", "label": "no target", "enabled": true},
        {"id": "e", "target": "host", "enabled": true},
    ]))
    .unwrap();
    let got = saved(&mut Fixed(Selection::All, rows));
    assert_eq!(labels(&got), ["ok", "e"]); // a missing label is the id
}
