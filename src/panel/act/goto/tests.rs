use super::*;
use crate::panel::act::fake::herdr;

#[tokio::test(flavor = "current_thread")]
async fn a_pane_is_there_or_gone() {
    let fake = herdr(&[("w1:p4", "w1:t4")]);
    assert!(exists(&fake.client(), "w1:p4").await);
    assert!(!exists(&fake.client(), "w1:p5").await);
    assert_eq!(gone("Billing"), "Billing is gone");
}
