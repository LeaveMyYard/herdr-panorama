use super::poll_lock;
use std::{
    cell::Cell,
    fs::TryLockError,
    io,
    time::{Duration, Instant},
};

const STEP: Duration = Duration::from_millis(25);

/// Runs `poll_lock` on a fake clock that only moves when it pauses; returns the result, the
/// attempts made and the fake time spent.
fn polled(
    timeout: Duration,
    answer: impl Fn(u32) -> Result<(), TryLockError>,
) -> (Result<(), String>, u32, Duration) {
    let start = Instant::now();
    let clock = Cell::new(start);
    let attempts = Cell::new(0);
    let out = poll_lock(
        || {
            attempts.set(attempts.get() + 1);
            answer(attempts.get())
        },
        timeout,
        || clock.get(),
        |pause| clock.set(clock.get() + pause),
    );
    (out, attempts.get(), clock.get() - start)
}

#[test]
fn a_busy_lock_is_retried_until_the_deadline_and_no_longer() {
    let (out, attempts, spent) = polled(Duration::from_secs(2), |_| Err(TryLockError::WouldBlock));
    assert!(out.unwrap_err().starts_with("runtime lock unavailable"));
    assert_eq!(spent, Duration::from_secs(2));
    assert_eq!(attempts, 81);
}

#[test]
fn a_zero_timeout_tries_once_and_never_pauses() {
    let (out, attempts, spent) = polled(Duration::ZERO, |_| Err(TryLockError::WouldBlock));
    assert!(out.is_err());
    assert_eq!((attempts, spent), (1, Duration::ZERO));
}

#[test]
fn a_lock_freed_during_the_wait_is_taken_at_once() {
    let (out, attempts, spent) = polled(Duration::from_secs(2), |n| {
        if n < 4 {
            Err(TryLockError::WouldBlock)
        } else {
            Ok(())
        }
    });
    assert_eq!(out, Ok(()));
    assert_eq!((attempts, spent), (4, 3 * STEP));
}

#[test]
fn any_other_lock_error_stops_the_wait() {
    let (out, attempts, spent) = polled(Duration::from_secs(2), |_| {
        Err(TryLockError::Error(io::Error::other("denied")))
    });
    assert!(out.unwrap_err().starts_with("runtime lock unavailable"));
    assert_eq!((attempts, spent), (1, Duration::ZERO));
}
