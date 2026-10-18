//! Subagents to any depth in the docked panel: the spawner tree's rows against design 5d's
//! render (`design_5d.json`, key `P4`), Python's tests/test_deep.py
//! `test_docked_panel_tree_rows_match_the_design`. The graph's depth columns (keys V4–V6)
//! come with the wide view in P4.

mod support;

use panorama_core::model::Status;
use panorama_core::view::palette::tile;
use panorama_core::view::{View, render_dock};
use support::{NO_SESSIONS, cells, fixture, golden, wire_swarm};

/// tests/design_deep.py `NOW`.
const NOW: f64 = 1_800_000_000.0;

#[test]
fn the_docked_tree_rows_are_the_designs() {
    let deep = fixture("design_5d.json");
    let mut sw = wire_swarm("inputs/swarm-deep.json", None);
    sw.docked = Some("w1:p3".into());
    let frames = deep["frames"].as_object().unwrap();
    assert!(!frames.is_empty());
    for (key, f) in frames {
        let frame: f64 = key.parse().unwrap();
        let v = View {
            orch: Some("w1:p3".into()),
            now: NOW,
            frame,
            ..View::default()
        };
        let s = render_dock(&sw, &v, 46, 45, None, NO_SESSIONS);
        let (want, got) = (golden(&f["P4"]), cells(&s));
        // the indented tree of the agents that spawned others (x >= 5: the design's thread is
        // not the last one, so its trunk column differs), and the thread row's agent glyphs
        for (y, x0) in std::iter::once((4, 34)).chain((5..10).map(|y| (y, 5))) {
            assert_eq!(want[y][x0..46], got[y][x0..46], "frame {key} row {y}");
        }
        // the thread's glyph in 5d sits in its tile in 9e
        let glyph = want[4][5].ch;
        let row4: String = (5..8).map(|x| s.cell(x, 4).ch).collect();
        assert_eq!(row4, format!(" {glyph} "), "frame {key}");
        assert_eq!(
            s.cell(6, 4).bg,
            Some(tile(&Status::Failed).0),
            "frame {key}"
        );
        let lead: String = (0..5).map(|x| s.cell(x, 4).ch).collect();
        assert_eq!(lead, "▌  └ ", "frame {key}"); // the only thread: its row closes the list
    }
}
