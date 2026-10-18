use super::*;
use crate::view::unseen::Unseen;

#[test]
fn glyphs_step_at_whole_frames_only() {
    assert_eq!(spin(0.0, 0), '⠋');
    assert_eq!(spin(0.99, 0), '⠋');
    assert_eq!(spin(1.0, 0), '⠙');
    assert_eq!(spin(3.0, 9), '⠹'); // (3 + 9) % 10 = 2
    assert_eq!(clock(3.9, 0), '⠁');
    assert_eq!(clock(4.0, 0), '⠈');
    assert_eq!(clock(0.0, 9), '⠈'); // seed 9 = 1 mod 8
}

#[test]
fn blink_is_on_eases_off_and_back() {
    assert_eq!(blink(0.0), 1.0);
    assert_eq!(blink(1.0), 1.0);
    assert!((blink(1.5) - 0.5).abs() < 1e-12);
    assert_eq!(blink(2.0), 0.0);
    assert_eq!(blink(3.0), 0.0);
    assert!((blink(3.5) - 0.5).abs() < 1e-12);
    assert_eq!(blink(4.0), 1.0);
    let on = Rgb::hex(0xffb547);
    assert_eq!(blink_faint(0.0, on), on);
    assert_eq!(blink_faint(2.5, on), FAINT);
}

#[test]
fn the_pulse_is_never_fully_off() {
    let base = Rgb::hex(0x101215);
    let m = Mark {
        event: Unseen::Wait,
        quiet: false,
    };
    assert_eq!(wave(0.0), 0.0);
    assert!((wave(6.0) - 1.0).abs() < 1e-12);
    assert_ne!(pulse(0.0, m, base, 1.0), base);
    assert_eq!(pulse(6.0, m, base, 1.0), Rgb::hex(0xffb547));
    assert_ne!(tint(0.0, Mark { quiet: true, ..m }, base), base);
}

use crate::model::Status;
use crate::model::status::{CLOCK, SPIN};
use crate::view::palette::{LINE, SEL};

fn rgb(c: Rgb) -> [i32; 3] {
    [i32::from(c.0), i32::from(c.1), i32::from(c.2)]
}

fn jump(a: Rgb, b: Rgb) -> i32 {
    let (a, b) = (rgb(a), rgb(b));
    (0..3).map(|i| (a[i] - b[i]).abs()).max().unwrap()
}

fn mark(event: Unseen, quiet: bool) -> Mark {
    Mark { event, quiet }
}

#[test]
fn whole_frames_are_the_stepped_design_frames() {
    // tests/test_smooth.py::test_whole_frames_are_the_stepped_design_frames
    let blinks: Vec<f64> = (0..8).map(|f| blink(f64::from(f))).collect();
    assert_eq!(blinks, [1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
    let fail = status(&Status::Failed);
    let cols: Vec<Rgb> = (0..4).map(|f| blink_faint(f64::from(f), fail)).collect();
    assert_eq!(cols, [fail, fail, FAINT, FAINT]);
    for f in 0..24 {
        let old = (1.0 - (2.0 * std::f64::consts::PI * f64::from(f % 12) / 12.0).cos()) / 2.0;
        assert_eq!(wave(f64::from(f)), old, "{f}");
    }
}

#[test]
fn colours_glide_in_small_steps_at_the_colour_tick() {
    // tests/test_smooth.py::test_colours_glide_in_small_steps_at_the_colour_tick: the colour
    // tick is 50 ms (COLOUR_SECONDS), a design frame 250 ms
    let step = 0.05 / 0.25;
    let ticks: Vec<f64> = (0..=(12.0 / step) as i32)
        .map(|i| f64::from(i) * step)
        .collect();
    let fail = status(&Status::Failed);
    type Case<'a> = (&'a dyn Fn(f64) -> Rgb, i32, usize);
    let cases: [Case; 3] = [
        (&|f| pulse(f, mark(Unseen::Fail, false), LINE, 1.0), 10, 25),
        (&|f| tint(f, mark(Unseen::Wait, false), SEL), 10, 12),
        (&|f| blink_faint(f, fail), 60, 5), // the blink eases over a frame
    ];
    for (i, (f, most, levels)) in cases.iter().enumerate() {
        let cols: Vec<Rgb> = ticks.iter().map(|&t| f(t)).collect();
        let worst = cols.windows(2).map(|w| jump(w[0], w[1])).max().unwrap();
        assert!(worst <= *most, "case {i}: {worst}");
        let distinct: std::collections::BTreeSet<[i32; 3]> = cols.iter().map(|&c| rgb(c)).collect();
        assert!(
            distinct.len() > *levels,
            "case {i}: {} levels",
            distinct.len()
        );
    }
    // the old 4 fps blink jumped the whole way at once
    assert!(jump(fail, FAINT) > 150);
}

#[test]
fn the_pulse_is_slow_and_never_off() {
    // tests/test_unseen.py::test_the_pulse_is_slow_and_never_off
    assert_eq!(PULSE_PERIOD, 12.0); // 3 s at 250 ms frames
    let full: Vec<Rgb> = (0..12)
        .map(|f| pulse(f64::from(f), mark(Unseen::Fail, false), LINE, 1.0))
        .collect();
    let quiet: Vec<Rgb> = (0..12)
        .map(|f| pulse(f64::from(f), mark(Unseen::Done, true), LINE, 1.0))
        .collect();
    assert!(!full.contains(&LINE) && !quiet.contains(&LINE));
    assert_eq!(full[6], status(&Status::Failed));
    assert_eq!(pulse(12.0, mark(Unseen::Fail, false), LINE, 1.0), full[0]);
}

#[test]
fn the_spinner_and_the_clock_are_braille_cycles() {
    // tests/test_glyph_spacing.py::test_the_spinner_is_the_ten_frame_braille_cycle and
    // tests/test_states.py::test_the_clock_turns_once_a_second_and_keeps_frames_running
    assert_eq!(SPIN.iter().collect::<String>(), "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
    assert_eq!(Status::Running.glyph(), "⠋");
    assert!(
        SPIN.iter()
            .chain(CLOCK.iter())
            .all(|&c| ('\u{2800}'..='\u{28ff}').contains(&c))
    );
    let steps: String = [0.0, 3.0, 4.0, 8.0, 28.0, 32.0]
        .iter()
        .map(|&f| clock(f, 0))
        .collect();
    assert_eq!(steps, "⠁⠁⠈⠐⠂⠁");
}

#[test]
fn colour_glides_stay_smooth_at_wall_clock_frames() {
    // tests/test_anim_sync.py::test_colour_glides_stay_smooth_at_wall_clock_frames: the frame
    // is about 7e9 (the time of day / 0.25 s); a pulse and a blink there match the same phase
    // near 0, and successive colour ticks move them in small steps
    let base = 600_000_000.0 * PULSE_PERIOD;
    assert_eq!(base % BLINK_PERIOD, 0.0);
    for i in 0..240 {
        let x = f64::from(i) * 0.05;
        assert!((wave(base + x) - wave(x)).abs() <= 1e-5, "wave {x}");
        assert!((blink(base + x) - blink(x)).abs() <= 1e-5, "blink {x}");
    }
    let step = 0.05 / 0.25;
    let steps = |f0: f64| -> Vec<f64> {
        let w: Vec<f64> = (0..240).map(|i| wave(f0 + f64::from(i) * step)).collect();
        w.windows(2).map(|p| p[1] - p[0]).collect()
    };
    for (a, b) in steps(base).iter().zip(steps(0.0)) {
        assert!((a - b).abs() <= 1e-5);
    }
}
