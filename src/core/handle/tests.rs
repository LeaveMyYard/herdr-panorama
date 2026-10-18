use super::*;
use crate::core::viewer::Viewer;
struct Rec(std::sync::Mutex<Vec<i32>>);
impl Verify for Rec {
    async fn verify(&self, _: &str, pid: i32) -> bool {
        self.0.lock().unwrap().push(pid);
        true
    }
}
#[tokio::test(flavor = "current_thread")]
async fn a_peer_with_pid_0_is_never_checked() {
    let v = Viewer::new(Some(0), 4);
    let r = Rec(Default::default());
    let (tx, _rx) = mpsc::channel(4);
    let msg = json!({"hello": {"protocol": "1.3", "pane_id": "w1:p1"}})
        .as_object()
        .unwrap()
        .clone();
    let by = Instant::now() + std::time::Duration::from_secs(1);
    assert_eq!(
        handle(&v, msg, &mut vec![], &r, by, &tx).await,
        Next::Welcomed
    );
    assert!(r.0.lock().unwrap().is_empty());
    assert_eq!(v.verified_pane(), None);
}
