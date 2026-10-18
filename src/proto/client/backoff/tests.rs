use super::*;

const MS: fn(u64) -> Duration = Duration::from_millis;

#[test]
fn the_schedule_is_pythons() {
    // panel.BACKOFF = (0.25, 0.5, 1.0, 2.0, 5.0)
    assert_eq!(BACKOFF, [MS(250), MS(500), MS(1000), MS(2000), MS(5000)]);
}

#[test]
fn waits_grow_then_repeat_the_last() {
    let mut b = Backoff::new(&BACKOFF);
    let waits: Vec<Duration> = (0..7).map(|_| b.next(false)).collect();
    assert_eq!(
        waits,
        [
            MS(250),
            MS(500),
            MS(1000),
            MS(2000),
            MS(5000),
            MS(5000),
            MS(5000)
        ]
    );
}

#[test]
fn a_welcomed_session_starts_over() {
    let mut b = Backoff::new(&BACKOFF);
    b.next(false);
    b.next(false);
    assert_eq!(b.next(true), MS(250));
    assert_eq!(b.next(false), MS(500));
}

#[test]
fn an_empty_schedule_is_pythons() {
    let mut b = Backoff::new(&[]);
    assert_eq!(b.next(false), MS(250));
}
