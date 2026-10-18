use super::*;

const MIN: Duration = Duration::from_secs(60);

#[test]
fn ten_minutes_without_a_core_and_the_panel_goes() {
    let t0 = Instant::now();
    let c = NoCoreClock::started(t0);
    assert!(!c.expired(t0 + 9 * MIN));
    assert!(c.expired(t0 + 10 * MIN));
}

#[test]
fn a_welcome_stops_the_clock_and_a_lost_core_starts_it_again() {
    let t0 = Instant::now();
    let mut c = NoCoreClock::started(t0);
    c.connected(true, t0 + 5 * MIN);
    assert!(!c.expired(t0 + 60 * MIN));
    c.connected(false, t0 + 30 * MIN);
    assert!(!c.expired(t0 + 39 * MIN));
    assert!(c.expired(t0 + 40 * MIN));
    // a second "lost" while already lost keeps the first start
    let mut c = NoCoreClock::started(t0);
    c.connected(false, t0 + 9 * MIN);
    assert!(c.expired(t0 + 10 * MIN));
}
