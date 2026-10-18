//! Panels for the input's tests: the design 9 swarm, captured in wire form.

use crate::panel::model::Panel;
use crate::proto::state::State;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// A state of the design 9 swarm with `extra` fields.
pub fn state(extra: Value) -> State {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut st = json!({ "swarm": fixture["swarm"] });
    st.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    State::read(st.as_object().unwrap()).unwrap()
}

/// A connected panel in `tab` that got a state showing `visible` at `t0`, painted once at
/// 46 columns (so it knows whether it is docked); returns it and a time when keys act.
pub fn panel(tab: &str, visible: &str) -> (Panel, Instant) {
    let t0 = Instant::now();
    let mut p = Panel::new(Some("w1:p90".into()), Some(tab.into()));
    p.on_connected(true);
    p.on_state(
        state(json!({"visible_tabs": [visible], "focused_pane": "w1:p90"})),
        t0,
    );
    let f = crate::panel::paint::Frame {
        now: 0.0,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    p.paint(46, 45, &f);
    (p, t0 + Duration::from_secs(1))
}
