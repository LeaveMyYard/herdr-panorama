use super::peer_allowed;

#[test]
fn only_a_peer_of_our_uid_is_allowed_and_an_unread_one_never() {
    assert!(peer_allowed(Some(1000), 1000));
    assert!(!peer_allowed(Some(1001), 1000));
    assert!(
        !peer_allowed(None, 1000),
        "a failed peer lookup must refuse"
    );
    assert!(!peer_allowed(None, 0));
}
