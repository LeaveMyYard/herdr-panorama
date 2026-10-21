//! Python's tests/test_anim_sync.py: animations are a function of the wall clock, not of a
//! per-window counter. Each actor paints at `clock::frame_at(now)`, so every window (the home
//! view, each tab's panel) shows the same phase at the same moment, however it got there.

use super::*;
use crate::panel::input::fixtures::{loaded_at, state};
use crate::ui::clock::{FRAME_SECONDS, frame_at};
use serde_json::json;

/// A wall-clock-sized instant, mid-frame (Python's `T`).
const T: f64 = 1_792_000_000.0 + 0.13;

impl Run {
    /// Paints at the wall-clock instant `t`, as the actor's colour tick does.
    fn tick(&mut self, t: f64) -> Screen {
        let (w, h) = self.size;
        let mut l = look(self.now_s);
        l.frame = frame_at(t);
        self.d.paint(w, h, &l, Painted::Other, self.t0).unwrap()
    }
}

fn cells(s: &Screen) -> Vec<(char, String, Option<String>, bool)> {
    (0..s.h())
        .flat_map(|y| (0..s.w()).map(move |x| (x, y)))
        .map(|(x, y)| {
            let c = s.cell(x, y);
            (c.ch, c.fg.to_string(), c.bg.map(|b| b.to_string()), c.bold)
        })
        .collect()
}

#[test]
fn two_windows_started_apart_show_the_same_phase() {
    // one window up for minutes, ticking fast; one started 2.3 s before T at 2 fps: at T both
    // draw the same picture, the one a window opened at T draws
    let size = (160, 45);
    let mut long = Run::demo("w1:p2", size);
    let mut t = T - 191.7;
    while t < T {
        long.tick(t);
        t += 0.05;
    }
    let mut late = Run::demo("w1:p2", size);
    let mut t = T - 2.3;
    while t < T {
        late.tick(t);
        t += 0.5;
    }
    let (a, b) = (long.tick(T), late.tick(T));
    let fresh = Run::demo("w1:p2", size).tick(T);
    assert_eq!(long.d.app.graph.view.frame, T / FRAME_SECONDS);
    assert_eq!(late.d.app.graph.view.frame, T / FRAME_SECONDS);
    assert_eq!(cells(&a), cells(&fresh));
    assert_eq!(cells(&b), cells(&fresh));
}

#[test]
fn a_panel_and_the_home_view_share_the_frame() {
    // a panel (another process in real use) started later and hidden in between reads the
    // frame the home view reads at the same instant
    let mut home = Run::demo("w1:p1", (160, 45));
    let mut t = T - 60.0;
    while t < T - 5.0 {
        home.tick(t);
        t += 0.25;
    }
    let (mut panel, t0) = loaded_at("w1:t3", "w1:t1"); // hidden
    home.tick(T);
    panel.on_state(
        state(json!({"visible_tabs": ["w1:t3"], "focused_pane": "w1:p90"})),
        t0,
    );
    let mut f = crate::panel::input::fixtures::frame();
    f.frame = frame_at(T);
    panel.paint(46, 45, &f);
    assert!(panel.tab_shown);
    assert_eq!(panel.graph.view.frame, home.d.app.graph.view.frame);
    assert_eq!(panel.graph.view.frame, T / FRAME_SECONDS);
}
