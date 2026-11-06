//! Real vendor marks use one title cell; targets and model labels remain unchanged.
use panorama_core::model::Swarm;
use panorama_core::view::harness::{CLAUDE, CODEX, HarnessIcons, title};
use panorama_core::view::text::trunc;
use panorama_core::view::{Target, View, render_dock, render_graph};
use serde_json::json;

fn swarm() -> Swarm {
    serde_json::from_value(json!({"roots":[
        {"pane_id":"claude-pane","tab_label":"same title","harness":"claude", "herdr_status":"working"},
        {"pane_id":"codex-pane","tab_label":"same title","harness":"codex", "herdr_status":"blocked"}
    ]})).unwrap()
}

#[test]
fn vendor_codepoints_and_text_fallback_are_exact() {
    let sw = swarm();
    assert_eq!(CLAUDE as u32, 0xe1a0);
    assert_eq!(CODEX as u32, 0xe1a1);
    for (o, glyph, text) in [
        (&sw.roots[0], CLAUDE, "Claude Code"),
        (&sw.roots[1], CODEX, "Codex"),
    ] {
        let name = title(o, HarnessIcons::Font);
        assert_eq!(name, format!("{glyph} same title"));
        assert_eq!(trunc(&name, 4), format!("{glyph} s…"));
        assert_eq!(title(o, HarnessIcons::Text), format!("{text} · same title"));
    }
    let mut o = sw.roots[0].clone();
    for harness in ["other", ""] {
        o.harness = harness.into();
        assert_eq!(
            title(&o, HarnessIcons::Font),
            format!("{harness} · same title")
        );
    }
    // Missing harness in old wire data still has the established Claude default.
    let sw: Swarm =
        serde_json::from_value(json!({"roots":[{"pane_id":"old","tab_label":"old tab"}]})).unwrap();
    assert_eq!(
        title(&sw.roots[0], HarnessIcons::Font),
        format!("{CLAUDE} old tab")
    );
}

#[test]
fn icon_titles_keep_layout_geometry_navigation_and_raw_labels() {
    let sw = swarm();
    let mut view = View {
        orch: Some("claude-pane".into()),
        harness_icons: HarnessIcons::Font,
        ..View::default()
    };
    for (dock, width) in [(true, 46), (true, 30), (false, 70), (false, 160)] {
        let screen = if dock {
            render_dock(&sw, &view, width, 32, None, "empty")
        } else {
            render_graph(&sw, &view, width, 32, None, "empty")
        };
        let text = screen.text();
        for (glyph, pane) in [(CLAUDE, "claude-pane"), (CODEX, "codex-pane")] {
            let positions: Vec<_> = text
                .lines()
                .enumerate()
                .flat_map(|(y, line)| {
                    line.chars()
                        .enumerate()
                        .filter_map(move |(x, ch)| (ch == glyph).then_some((x as i32, y as i32)))
                })
                .collect();
            assert_eq!(positions.len(), 1, "{dock}/{width}: {text}");
            let (x, y) = positions[0];
            assert_eq!(screen.cell(x + 1, y).ch, ' ');
            assert_eq!(screen.cell(x + 2, y).ch, 's');
            assert_eq!(screen.target_at(x, y), Some(&Target::Orch(pane.into())));
            assert_eq!(screen.target_at(x + 2, y), Some(&Target::Orch(pane.into())));
        }
        // Font mode changes title cells only, never layout regions or hit boxes.
        view.harness_icons = HarnessIcons::Text;
        let plain = if dock {
            render_dock(&sw, &view, width, 32, None, "empty")
        } else {
            render_graph(&sw, &view, width, 32, None, "empty")
        };
        assert_eq!(screen.targets, plain.targets);
        assert_eq!(screen.columns, plain.columns);
        view.harness_icons = HarnessIcons::Font;
    }
    assert!(sw.roots.iter().all(|o| o.tab_label == "same title"));
}
