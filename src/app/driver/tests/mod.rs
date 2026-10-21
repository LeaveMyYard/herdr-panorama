//! The app's keys and pointer through its [`Driver`], frame by frame as the actor drives it:
//! Python's tests/test_mouse.py, the input half of tests/test_tui.py and tests/test_deep.py.
//! Where Python asserted a herdr call (`agent focus`), these assert the [`Effect`] that asks
//! for it; the actor carries it out (P4 8c).

mod arming;
mod clicks;
mod deep;
mod dock;
mod focus;
mod frames;
mod keys;
mod narrow;
mod phase;
mod refresh;
mod wheel;

use super::*;
use crate::app::model::App;
use crate::ui::input::Effect;
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, Target};
use serde_json::Value;
use std::time::Duration;

/// A captured wire swarm (`tests/fixtures/inputs/swarm-<name>.json`) and its clock.
fn wire(name: &str) -> (Swarm, f64) {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    (
        serde_json::from_value(v["swarm"].clone()).unwrap(),
        v["now"].as_f64().unwrap(),
    )
}

/// An app as tests drive it: its driver, the instant of its first refresh, its size.
struct Run {
    d: Driver,
    t0: Instant,
    now_s: f64,
    size: (i32, i32),
}

/// The frame at the run's clock, panorama's own colours.
fn look(now: f64) -> Frame {
    Frame {
        now,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    }
}

impl Run {
    /// The app on `swarm` opened from `pane` (`home`: the home view), after its first
    /// refresh and paint at `size`.
    fn new(swarm: (Swarm, f64), pane: &str, home: bool, size: (i32, i32)) -> Self {
        let t0 = Instant::now();
        let mut app = App::new(Some(pane.into()), home, t0);
        app.apply(swarm.0, Marks::new(), t0);
        let mut run = Self {
            d: Driver::new(app),
            t0,
            now_s: swarm.1,
            size,
        };
        run.paint(Painted::Refresh, t0);
        run
    }

    fn demo(pane: &str, size: (i32, i32)) -> Self {
        Self::new(wire("demo"), pane, false, size)
    }

    fn paint(&mut self, cause: Painted, now: Instant) {
        let (w, h) = self.size;
        self.d.paint(w, h, &look(self.now_s), cause, now);
    }

    /// As a second later: nothing just arrived, the keys act (Python `settle`).
    fn settled(&mut self) -> Instant {
        self.d.app.armed_at = self.t0;
        self.t0 + Duration::from_secs(5)
    }

    /// A refresh with `swarm` at `now`, painted.
    fn refresh(&mut self, swarm: Swarm, now: Instant) {
        self.d.app.apply(swarm, Marks::new(), now);
        self.paint(Painted::Refresh, now);
    }

    /// A cell where a click hits exactly `target`.
    fn spot(&self, target: &Target) -> Option<(i32, i32)> {
        let s = self.d.shown().unwrap();
        s.targets.iter().find_map(|h| {
            (h.target == *target)
                .then(|| {
                    (h.x0..h.x1)
                        .find(|x| s.target_at(*x, h.y) == Some(target))
                        .map(|x| (x, h.y))
                })
                .flatten()
        })
    }

    /// A click at `(x, y)` at `now`, then the repaint the actor does.
    fn click_at(&mut self, (x, y): (i32, i32), now: Instant) -> Vec<Effect> {
        let (out, cause) = self.d.pointer(Pointer::Click, x, y, now);
        if let Some(cause) = cause {
            self.paint(cause, now);
        }
        out
    }

    /// A click on `target` at `now`.
    fn click(&mut self, target: &Target, now: Instant) -> Vec<Effect> {
        let at = self
            .spot(target)
            .unwrap_or_else(|| panic!("{target:?} is not drawn"));
        self.click_at(at, now)
    }

    /// A key at `now`, then the repaint.
    fn key(&mut self, k: Key, now: Instant) -> Vec<Effect> {
        let out = self.d.key(k, now);
        self.paint(Painted::Other, now);
        out
    }

    /// `(focus, orch, thread, agent)` (test_tui.py `sel`).
    fn sel(&self) -> (usize, Option<&str>, Option<&str>, Option<&str>) {
        let v = &self.d.app.view;
        (
            v.focus,
            v.orch.as_deref(),
            v.thread.as_deref(),
            v.agent.as_deref(),
        )
    }

    /// The text on screen.
    fn text(&self) -> String {
        self.d.shown().unwrap().text()
    }
}

/// The orchestrator target of `pane`.
fn orch(pane: &str) -> Target {
    Target::Orch(pane.into())
}

/// Whether `out` goes to `pane` (Python's `agent focus <pane>`).
fn goes_to(out: &[Effect], pane: &str) -> bool {
    out.iter()
        .any(|e| matches!(e, Effect::Goto { pane: p, .. } if p == pane))
}
