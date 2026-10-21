//! Python's tests/test_machines.py::test_enter_on_a_remote_orchestrator_focuses_it_there_and_says_where:
//! herdr cannot switch this view to another machine, so acting on a remote orchestrator asks
//! for its pane on that machine's own server (by the machine's id) and says where it is:
//! never a dock, never a local focus. (The call itself and the words are
//! `panel::act::remote`'s test.) Python runs it in the home view with the moving cockpit off
//! and on; the cockpit is gone (Q187), and `g` in a popup is `keys.rs`'s test.

use super::*;

const WIDE: (i32, i32) = (160, 45);

#[test]
fn acting_on_a_remote_orchestrator_asks_its_own_server_never_a_local_focus() {
    {
        let home = true; // the home view (Python's)
        let mut r = Run::new(wire("remote"), "abc123:w1:p1", home, WIDE);
        let now = r.settled();
        assert_eq!(r.d.app.view.orch.as_deref(), Some("abc123:w1:p1")); // the same local ids, another identity
        // a: answering there is not supported; it says so (Python's first note)
        let out = r.key(Key::Answer, now);
        assert!(
            matches!(out.as_slice(), [Effect::Toast(t)] if t.text.contains("answering") && t.text.contains("devbox")),
            "{home}: {out:?}"
        );
        // g: its pane on its own server, by the machine's id
        r.d.app.armed_at = r.t0;
        let out = r.key(Key::Goto, now);
        let remote: Vec<_> = out
            .iter()
            .filter_map(|e| match e {
                Effect::GotoRemote {
                    machine_id,
                    machine,
                    pane,
                    ..
                } => Some((machine_id.as_str(), machine.as_str(), pane.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            remote,
            [("abc123", "devbox", "abc123:w1:p1")],
            "{home}: {out:?}"
        );
        assert!(
            !goes_to(&out, "abc123:w1:p1") && !goes_to(&out, "w1:p1"),
            "{home}: {out:?}"
        );
    }
}
