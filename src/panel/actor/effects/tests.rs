use super::*;

#[tokio::test(flavor = "current_thread")]
async fn a_filter_send_is_waited_for_at_most_its_bound() {
    assert_eq!(bounded(async { true }, FILTER_SEND).await, Some(true));
    assert_eq!(bounded(async { false }, FILTER_SEND).await, Some(false)); // no core
    // a core that stopped reading: the actor goes on after the bound, and the filter (queued
    // in the client) is not taken back
    let (t, within) = (Instant::now(), Duration::from_millis(50));
    assert_eq!(bounded(std::future::pending(), within).await, None);
    assert!(t.elapsed() >= within && t.elapsed() < FILTER_SEND);
}
