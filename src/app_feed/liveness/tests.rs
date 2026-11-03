use super::*;
#[test]
fn idle_requires_thirty_seconds_of_confirmed_absence_and_unknown_resets_it() {
    let mut idle = Idle::default();
    assert!(!idle.observe(State::Absent, Duration::ZERO));
    assert!(!idle.observe(State::Absent, Duration::from_secs(29)));
    assert!(idle.observe(State::Absent, Duration::from_secs(30)));
    assert!(!idle.observe(State::Running, Duration::from_secs(31)));
    assert!(!idle.observe(State::Absent, Duration::from_secs(32)));
    assert!(!idle.observe(State::Unknown, Duration::from_secs(61)));
    assert!(!idle.observe(State::Absent, Duration::from_secs(62)));
    assert!(!idle.observe(State::Absent, Duration::from_secs(91)));
    assert!(idle.observe(State::Absent, Duration::from_secs(92)));
}
