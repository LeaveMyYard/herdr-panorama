//! The panel against its app-screen golden (`tests/fixtures/screens/panel.json`, captured
//! headless from Python's `PanelApp`): before the first state, shown, the core's filter, and
//! no core. The run's steps drive the panel's model as Python's capture drove the app. The
//! panel docks next to an agent, so its screens are Rust's own since the docked list became
//! the root's agent tree (plan `uniform-agent-tree.md` U5, D5): `rust/screens/panel.json`,
//! rewritten by `PANORAMA_BLESS=1 cargo test --test panel_screens` and reviewed as a diff.

use panorama::panel::model::Panel;
use panorama::panel::paint::Frame;
use panorama::proto::state::State;
mod screen_support;

use panorama_core::view::{PanelSettings, Tokens};
use screen_support::bless::blessing;
use screen_support::{cells, fixture, golden, runs};
use serde_json::json;

#[test]
fn the_panels_screens_are_pythons() {
    let file = fixture("screens/panel.json");
    let run = &file["runs"][0];
    let mut panel = Panel::new(
        run["pane_id"].as_str().map(Into::into),
        run["tab_id"].as_str().map(Into::into),
    );
    // no herdr config: herdr's default theme, panorama's fixed colours, the open key unbound
    let mut f = Frame {
        now: run["now"].as_f64().unwrap(),
        frame: 0.0,
        utc_offset: 0, // TZ=UTC
        theme: Tokens::builtin("catppuccin"),
        settings: PanelSettings::default(),
        harness_icons: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let rust = if blessing() {
        serde_json::Value::Null
    } else {
        fixture("rust/screens/panel.json")
    };
    let mut blessed = serde_json::Map::new();
    let mut snaps = 0;
    for step in run["steps"].as_array().unwrap() {
        match step["do"].as_str().unwrap() {
            "core" => panel.on_connected(step["connected"].as_bool().unwrap()),
            "state" => {
                let swarm = fixture(step["swarm"].as_str().unwrap())["swarm"].clone();
                let mut st = json!({"swarm": swarm, "visible_tabs": step["visible"]});
                if let Some(name) = step.get("filter") {
                    st["filter"] = name.clone();
                }
                panel.on_state(
                    State::read(st.as_object().unwrap()).unwrap(),
                    std::time::Instant::now(),
                );
            }
            "frame" => f.frame = step["n"].as_f64().unwrap(),
            "snap" => {
                let name = step["name"].as_str().unwrap();
                let (w, h) = (
                    run["size"][0].as_i64().unwrap() as i32,
                    run["size"][1].as_i64().unwrap() as i32,
                );
                let screen = panel.paint(w, h, &f);
                snaps += 1;
                if blessing() {
                    blessed.insert(name.to_owned(), runs(&screen));
                    continue;
                }
                let want = golden(&rust["screens"][name]);
                let got = cells(&screen);
                for (y, (a, b)) in want.iter().zip(&got).enumerate() {
                    assert_eq!(a, b, "{name}: row {y} differs");
                }
                assert_eq!(want.len(), got.len(), "{name}: rows");
            }
            other => panic!("step {other}"),
        }
    }
    assert_eq!(snaps, 4);
    if blessing() {
        let doc = json!({
            "source": "Rust's screens for the run of tests/fixtures/screens/panel.json, written \
                       by PANORAMA_BLESS=1 cargo test --test panel_screens",
            "screens": blessed,
        });
        let path = format!(
            "{}/tests/fixtures/rust/screens/panel.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
}
