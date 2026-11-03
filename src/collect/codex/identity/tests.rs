use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};

fn identity(fake: &FakeHerdr, dir: &std::path::Path) -> LocalIdentity {
    let env = PluginEnv::from_vars(
        [("XDG_STATE_HOME", dir.as_os_str())],
        Some(dir.to_owned()),
        1000,
    );
    LocalIdentity::new(env, fake.client().blocking())
}

#[test]
fn native_identity_and_absent_store_do_not_query_processes() {
    let fake = FakeHerdr::start();
    let dir = tempfile::tempdir().unwrap();
    let mut identity = identity(&fake, dir.path());
    let id = "10000000-0000-4000-8000-000000000001";
    assert_eq!(identity.resolve("w1:p1", Some(id)).as_deref(), Some(id));
    assert_eq!(identity.resolve("w1:p1", None), None);
    assert_eq!(identity.resolve("w1:p1", Some("bad-id")), None);
    assert!(fake.calls().is_empty());
}

#[test]
fn per_collect_cache_is_reused_only_until_next_foreground_snapshot() {
    let fake = FakeHerdr::start();
    let dir = tempfile::tempdir().unwrap();
    let mut identity = identity(&fake, dir.path());
    fake.on("pane.process_info", Reply::Result(json!({"generation":1})));
    assert_eq!(identity.process("w1:p1"), Some(json!({"generation":1})));
    fake.on("pane.process_info", Reply::Result(json!({"generation":2})));
    assert_eq!(identity.process("w1:p1"), Some(json!({"generation":1})));
    assert_eq!(fake.calls().len(), 1);
    identity.begin_collect();
    assert_eq!(identity.process("w1:p1"), Some(json!({"generation":2})));
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn hung_process_lookup_has_one_budget_for_all_panes() {
    let fake = FakeHerdr::start();
    fake.on("pane.process_info", Reply::Hang);
    let dir = tempfile::tempdir().unwrap();
    let mut identity = identity(&fake, dir.path());
    let start = Instant::now();
    for index in 0..40 {
        assert!(identity.process(&format!("w1:p{index}")).is_none());
    }
    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(fake.calls().len() <= 2);
}

#[test]
fn unrelated_elapsed_time_does_not_consume_lookup_budget() {
    let fake = FakeHerdr::start();
    let dir = tempfile::tempdir().unwrap();
    let mut identity = identity(&fake, dir.path());
    fake.on(
        "pane.process_info",
        Reply::Result(json!({"process_info":{}})),
    );
    identity.begin_collect();
    std::thread::sleep(Duration::from_millis(120));
    assert!(identity.process("pane").is_some());
    assert_eq!(fake.calls().len(), 1);
}
