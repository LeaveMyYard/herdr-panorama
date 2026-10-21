//! tests/test_tui.py's animation frames: a frame repaints from the model the app has (no
//! collect), moves only what animates, never re-arms the keys or moves the cursor, and runs
//! unfocused; with nothing animated nothing ticks.

use super::*;
use crate::ui::pacing::ticking;

const WIDE: (i32, i32) = (160, 45);

impl Run {
    /// A colour tick's frame `f` (the actor's `animate`: a paint, no cause to stamp).
    fn frame(&mut self, f: f64, now: Instant) -> String {
        let (w, h) = self.size;
        let mut l = look(self.now_s);
        l.frame = f;
        self.d.paint(w, h, &l, Painted::Other, now).unwrap().text()
    }
}

#[test]
fn spinner_frames_advance_without_changing_width() {
    let mut r = Run::demo("w1:p2", WIDE);
    let (first, second) = (r.frame(0.0, r.t0), r.frame(1.0, r.t0));
    assert_ne!(first, second);
    let widths = |t: &str| t.lines().map(|l| l.chars().count()).collect::<Vec<_>>();
    assert_eq!(r.d.shown().unwrap().w(), WIDE.0);
    assert_eq!(widths(&first).len(), widths(&second).len());
    let o = &r.d.app.swarm.orchestrators[1];
    assert!(
        o.agents()
            .any(|a| a.status == panorama_core::model::Status::Running)
    );
}

#[test]
fn frames_never_rearm_or_move_the_cursor() {
    // test_tui.py::test_frames_never_recollect_rearm_or_move_the_cursor (a frame is a paint of
    // the model the app holds: the actor asks for a collect only on its refresh tick)
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.key(Key::Right, now);
    let (before, swarm) = (format!("{:?}", r.sel()), r.d.app.swarm.clone());
    for f in 1..=12 {
        r.frame(f64::from(f), now);
    }
    assert_eq!(format!("{:?}", r.sel()), before);
    assert_eq!(r.d.app.swarm, swarm);
    assert!(r.d.app.armed(now)); // not a background change
    r.key(Key::Move(1), now); // keys keep working at once
    assert_eq!(r.d.app.view.thread.as_deref(), Some("ref:#44"));
    assert!(goes_to(&r.key(Key::Goto, now), "w1:p1"));
}

#[test]
fn frames_keep_running_unfocused_and_pause_when_nothing_moves() {
    let mut r = Run::demo("w1:p1", WIDE);
    r.d.app.pane_focused = false; // herdr's focus-out: the operator types next door
    let moving = |r: &Run| ticking(true, &r.d.app.swarm, &r.d.app.view.unseen); // the actor's
    assert!(moving(&r));
    assert_ne!(r.frame(1.0, r.t0), r.frame(2.0, r.t0)); // still animating
    r.refresh(Swarm::default(), r.t0); // nothing animated on screen
    assert!(!moving(&r)); // the tick does not even wake
}
