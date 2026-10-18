use super::*;
use herdr_agent_kit::herdr_api::fake::{CliReply, CliStub, FakeHerdr};

#[test]
fn a_namespaced_pane_id_back_to_its_own_servers() {
    assert_eq!(local_id("m1:w1:p3", "m1"), "w1:p3");
    assert_eq!(local_id("w1:p3", "m1"), "w1:p3");
}

#[tokio::test(flavor = "current_thread")]
async fn the_pane_is_focused_on_its_machine_and_the_operator_told() {
    let fake = FakeHerdr::start();
    let mut stub = CliStub::new();
    stub.on(
        &["--machine", "m1", "agent", "focus", "w1:p3"],
        CliReply::ok("{}"),
    );
    let h = fake.client().with_bin(stub.bin());
    let text = goto_remote(&h, "m1", "box", "m1:w1:p3", "Remote work").await;
    assert_eq!(text, "Remote work is on box: switch herdr to box to see it");
    let failed = goto_remote(&h, "m2", "other", "m2:w1:p3", "x").await;
    assert!(
        failed.starts_with("could not focus it on other: "),
        "{failed}"
    );
}
