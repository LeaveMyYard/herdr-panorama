use super::*;

#[test]
fn the_animation_time_is_the_wall_clock_in_quarter_seconds() {
    // tests/test_core.py::test_a_hidden_tabs_panel_draws_no_frames (shown again, a panel jumps
    // straight to the clock's phase) and test_anim_sync.py: every window's frame is now / 0.25 s
    assert_eq!(FRAME_SECONDS, 0.25);
    assert_eq!(frame_at(1_800_000_000.0), 7_200_000_000.0);
    let later = frame_at(1_800_000_000.3) - frame_at(1_800_000_000.0);
    assert!((later - 1.2).abs() < 1e-5, "{later}");
}
