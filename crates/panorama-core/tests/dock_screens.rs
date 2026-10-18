//! The docked panel against the app-screen goldens (tests/fixtures/README.md, "Behaviour
//! goldens"): the home app docked next to an agent, captured headless from Python. Only the
//! snaps a pure render reproduces are here (a key's effect on the cursor is given as the
//! view the app was left with; the key handling itself is the panel app's to test). Fixed by the capture: no herdr config (panorama's fixed colours), the open key
//! unbound, frame 0 unless the name says otherwise.

mod support;

use panorama_core::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG};
use panorama_core::view::theme::{OwnColours, PanelSettings, panel_look};
use panorama_core::view::{Target, View, render_dock};
use support::{assert_same, cells, fixture, golden, wire_swarm};

const OWN: OwnColours = OwnColours {
    fg: FG,
    dim: DIM,
    faint: FAINT,
    fallback: (DOCK_HOV, BG),
};

/// The run's app as it paints: home mode docked (`z` zooms), the cursor on the focus pane.
fn app_view(run: &serde_json::Value, focused: bool) -> View {
    View {
        orch: run["focus_pane"].as_str().map(Into::into),
        zoom_hint: true,
        now: run["now"].as_f64().unwrap(),
        panel: Some(panel_look(None, focused, OWN, &PanelSettings::default())),
        ..View::default()
    }
}

fn check(file: &str, snap: &str, view: impl FnOnce(&serde_json::Value) -> View) {
    let golden_file = fixture(&format!("screens/{file}"));
    let run = &golden_file["runs"][0];
    let swarm = wire_swarm(run["input"].as_str().unwrap(), run["docked"].as_str());
    let size = &run["size"];
    let (w, h) = (
        size[0].as_i64().unwrap() as i32,
        size[1].as_i64().unwrap() as i32,
    );
    let got = render_dock(&swarm, &view(run), w, h, None);
    assert_same(
        &format!("{file} {snap}"),
        &golden(&golden_file["screens"][snap]),
        &cells(&got),
    );
}

#[test]
fn docked_focused() {
    check("dock.json", "focused", |run| app_view(run, true));
}

#[test]
fn docked_with_the_mouse_over_an_entry_at_frame_4() {
    check("dock.json", "hover@frame4", |run| View {
        hover: Some(Target::Orch("w1:p4".into())),
        frame: 4.0,
        ..app_view(run, true)
    });
}

#[test]
fn docked_unfocused() {
    // the run pressed down before it: the cursor is on the next entry (Billing Migrations)
    check("dock.json", "unfocused", |run| View {
        orch: Some("w1:p4".into()),
        ..app_view(run, false)
    });
}

#[test]
fn remote_machines_get_a_divider_and_no_tab_number() {
    check("remote-dock.json", "initial", |run| app_view(run, true));
}
