use super::*;

#[test]
fn a_panic_in_a_task_is_its_result_and_the_hook_knows_it() {
    let _hook = hook_lock();
    let hook = std::panic::take_hook();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let s = seen.clone();
    std::panic::set_hook(Box::new(move |info| {
        // only this test's panic (others may run meanwhile)
        if info.payload().downcast_ref::<&str>() == Some(&"guarded boom") {
            s.lock().unwrap().push(in_task());
        }
    }));
    let out: Result<(), String> = block_on(guarded(async {
        tokio::task::yield_now().await; // a panic after a wait, as a herdr call's
        panic!("guarded boom");
    }));
    std::panic::set_hook(hook);
    assert_eq!(out, Err("guarded boom".to_owned()));
    assert_eq!(*seen.lock().unwrap(), [true]);
    assert!(!in_task());
    assert_eq!(block_on(guarded(async { 7 })), Ok(7));
}
