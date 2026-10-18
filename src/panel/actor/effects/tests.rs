use super::*;

#[tokio::test(flavor = "current_thread")]
async fn a_filter_the_core_does_not_take_in_time_is_not_sent() {
    assert!(bounded(async { true }, FILTER_SEND).await);
    assert!(!bounded(async { false }, FILTER_SEND).await);
    // a core that stopped reading: the send never ends, the actor goes on after the bound
    let (t, within) = (Instant::now(), Duration::from_millis(50));
    assert!(!bounded(std::future::pending(), within).await);
    assert!(t.elapsed() >= within && t.elapsed() < FILTER_SEND);
}
