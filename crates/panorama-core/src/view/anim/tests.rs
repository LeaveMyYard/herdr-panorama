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
