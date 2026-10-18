//! The docked panel against the design's own renders (tests/fixtures/README.md, "Design
//! references"), cell for cell: design 9e (`design_9.json`, plain and with turn 6's unseen
//! marks) and turn 10 (`design_10.json`, focused and not, in two herdr themes). Python's
//! tests/test_tiles.py and tests/test_focus_theme.py, ported.

mod support;

use panorama_core::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG};
use panorama_core::view::theme::{OwnColours, PanelSettings, Tokens, panel_look};
use panorama_core::view::{Marks, Target, Unseen, View, render_dock};
use support::{assert_same, cells, fixture, golden, wire_swarm};

/// The design's dataset in wire form, docked next to tab 3, as in the design.
fn design_swarm() -> panorama_core::model::Swarm {
    wire_swarm("inputs/swarm-design9.json", Some("w1:p3"))
}

/// The design's view: tab 3's entry selected, the mouse over Billing Migrations.
fn design_view(frame: f64) -> View {
    View {
        orch: Some("w1:p3".into()),
        hover: Some(Target::Orch("w1:p4".into())),
        frame,
        ..View::default()
    }
}

/// Turn 6's sample marks (test_tiles.py `MARKS`).
fn marks() -> Marks {
    [
        ("b3", Unseen::Fail),
        ("c1", Unseen::Wait),
        ("a5", Unseen::Done),
        ("i3", Unseen::Wait),
        ("k2", Unseen::Done),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn frames(golden_file: &serde_json::Value) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = golden_file["frames"]
        .as_object()
        .unwrap()
        .keys()
        .map(|k| (k.clone(), k.parse().unwrap()))
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert!(!out.is_empty());
    out
}

#[test]
fn design_9e_cell_for_cell() {
    let nine = fixture("design_9.json");
    let swarm = design_swarm();
    for (key, frame) in frames(&nine) {
        let got = render_dock(&swarm, &design_view(frame), 46, 45, None);
        assert_same(
            &format!("9e frame {key}"),
            &golden(&nine["frames"][&key]["E"]),
            &cells(&got),
        );
    }
}

#[test]
fn design_9e_unseen_tiles_pulse_like_the_design() {
    let nine = fixture("design_9.json");
    let swarm = design_swarm();
    for (key, frame) in frames(&nine) {
        let view = View {
            unseen: marks(),
            ..design_view(frame)
        };
        let got = render_dock(&swarm, &view, 46, 45, None);
        assert_same(
            &format!("9e unseen frame {key}"),
            &golden(&nine["frames"][&key]["U"]),
            &cells(&got),
        );
    }
}

const OWN: OwnColours = OwnColours {
    fg: FG,
    dim: DIM,
    faint: FAINT,
    fallback: (DOCK_HOV, BG),
};

#[test]
fn the_design_uses_our_theme_table() {
    let ten = fixture("design_10.json");
    let tokyo = &ten["themes"]["tokyo"];
    let want = Tokens::builtin("tokyo-night").unwrap();
    for (name, got) in [
        ("panel_bg", want.panel_bg),
        ("active_row_bg", want.active_row_bg),
        ("text", want.text),
        ("subtext0", want.subtext0),
        ("overlay1", want.overlay1),
        ("overlay0", want.overlay0),
    ] {
        assert_eq!(
            tokyo[name].as_str(),
            got.map(|c| c.to_string()).as_deref(),
            "{name}"
        );
    }
    let yours = &ten["themes"]["yours"];
    assert_eq!(yours["focus_bg"], "#14161b");
    assert_eq!(yours["panel_bg"], "#101215");
}

#[test]
fn design_10_focused_and_not_in_two_themes_cell_for_cell() {
    let ten = fixture("design_10.json");
    let swarm = design_swarm();
    let tokyo = Tokens::builtin("tokyo-night").unwrap();
    let herdr = PanelSettings {
        follow_herdr: true,
        ..PanelSettings::default()
    };
    for (key, frame) in frames(&ten) {
        // "yours": the fixed default (no config); "tokyo": panel_colors = "herdr" with tokyo-night
        for (theme, tokens, settings) in [
            ("yours", None, PanelSettings::default()),
            ("tokyo", Some(&tokyo), herdr),
        ] {
            for (mode, focused) in [("A", true), ("I", false)] {
                let view = View {
                    panel: Some(panel_look(tokens, focused, OWN, &settings)),
                    ..design_view(frame)
                };
                let got = render_dock(&swarm, &view, 46, 45, None);
                let want = golden(&ten["frames"][&key][format!("{theme}{mode}")]);
                assert_same(
                    &format!("turn 10 frame {key} {theme}{mode}"),
                    &want,
                    &cells(&got),
                );
            }
        }
    }
}
