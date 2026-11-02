use super::*;
use std::cell::Cell;

fn swarm(revision: usize) -> Swarm {
    Swarm {
        generated_at: revision as f64,
        ..Swarm::default()
    }
}

#[test]
fn ready_or_unavailable_observations_do_not_wait() {
    let result = with_clock(
        || (swarm(1), false),
        BUDGET,
        || Duration::ZERO,
        |_| panic!("no pending work should wait"),
    );
    assert_eq!(result.generated_at, 1.0);
}

#[test]
fn pending_work_refreshes_until_ready_with_no_real_test_sleep() {
    let elapsed = Cell::new(Duration::ZERO);
    let calls = Cell::new(0);
    let result = with_clock(
        || {
            calls.set(calls.get() + 1);
            (swarm(calls.get()), calls.get() < 3)
        },
        BUDGET,
        || elapsed.get(),
        |duration| elapsed.set(elapsed.get() + duration),
    );
    assert_eq!(calls.get(), 3);
    assert_eq!(result.generated_at, 3.0);
    assert_eq!(elapsed.get(), POLL * 2);
}

#[test]
fn one_deadline_bounds_all_pending_panes_and_keeps_last_model() {
    let elapsed = Cell::new(Duration::ZERO);
    let calls = Cell::new(0);
    let result = with_clock(
        || {
            calls.set(calls.get() + 1);
            (swarm(calls.get()), true)
        },
        Duration::from_millis(25),
        || elapsed.get(),
        |duration| elapsed.set(elapsed.get() + duration),
    );
    assert_eq!(elapsed.get(), Duration::from_millis(25));
    assert_eq!(calls.get(), 3);
    assert_eq!(result.generated_at, 3.0);
}

#[test]
fn collection_time_consumes_the_same_deadline() {
    let elapsed = Cell::new(Duration::ZERO);
    let result = with_clock(
        || {
            elapsed.set(BUDGET);
            (swarm(1), true)
        },
        BUDGET,
        || elapsed.get(),
        |_| panic!("deadline already exhausted"),
    );
    assert_eq!(result.generated_at, 1.0);
}
