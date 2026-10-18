use super::*;
use serde_json::json;

const ID: &str = "leavemyyard.herdr-panorama";

fn cfg(keys: Value) -> Map<String, Value> {
    json!({ "keys": { "command": keys } })
        .as_object()
        .unwrap()
        .clone()
}

#[test]
fn short_forms() {
    assert_eq!(short("prefix+space"), "pfx+spc");
    assert_eq!(short("ctrl+alt+p"), "c-a-p");
    assert_eq!(short("Prefix+Shift+Enter"), "pfx+s-ret");
    assert_eq!(short("f5"), "f5");
    // test_open_key.py's cases
    assert_eq!(short("prefix+alt+g"), "pfx+a-g");
    assert_eq!(short("ctrl+shift+enter"), "c-s-ret");
}

#[test]
fn the_binding_is_the_plugin_action_for_our_open() {
    let keys = json!([
        {"type": "plugin_action", "command": "other.open", "key": "ctrl+o"},
        {"type": "shell", "command": "leavemyyard.herdr-panorama.open", "key": "ctrl+x"},
        {"type": "plugin_action", "command": "leavemyyard.herdr-panorama.open", "key": " ctrl+alt+p "},
    ]);
    assert_eq!(
        current(&cfg(keys), ID),
        ("ctrl+alt+p".to_owned(), "c-a-p".to_owned())
    );
    assert_eq!(
        current(&cfg(json!([])), ID),
        ("pfx+spc".to_owned(), "pfx+spc".to_owned())
    );
    let suggested = json!([{"type": "plugin_action", "command": "leavemyyard.herdr-panorama.open", "key": "Prefix+Space"}]);
    assert_eq!(current(&cfg(suggested), ID).0, "pfx+spc");
    // another plugin id (HERDR_PLUGIN_ID) names another action
    let other = json!([{"type": "plugin_action", "command": "fork.open", "key": "f9"}]);
    assert_eq!(current(&cfg(other), "fork").0, "f9");
    assert_eq!(binding(&Map::new(), ID), None);
}
