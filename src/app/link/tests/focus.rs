//! herdr's focus and the tab it shows, as the home view and its panels see them.

use super::{done, link, quiet};
use crate::app::link::Done;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn the_view_is_focused_exactly_when_herdr_focuses_its_own_pane() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut l = link(&fake, true, dir.path());
    assert_eq!(l.herdr_focus("w1:p1"), Some(true));
    assert_eq!(l.herdr_focus("w1:p2"), Some(false));
    assert_eq!(
        l.published.focused_pane.as_deref(),
        Some("w1:p2"),
        "published to the panels"
    );
    assert_eq!(l.herdr_focus(""), None, "no pane: nothing known");
    l.my_pane = None;
    assert_eq!(
        l.herdr_focus("w1:p1"),
        None,
        "without its own pane the view cannot say"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_tab_shown_again_is_no_change() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut l = link(&fake, true, dir.path());
    assert!(l.tab_shown("w1:t2".into()));
    assert!(!l.tab_shown("w1:t2".into()));
    assert!(l.tab_shown("w1:t3".into()));
    assert_eq!(l.published.visible_tab.as_deref(), Some("w1:t3"));
}

#[tokio::test(flavor = "current_thread")]
async fn an_announced_focus_is_read_again_unless_herdr_confirmed_one() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot": {"focused_pane_id": "w1:p7", "panes": []}})),
    );
    let mut l = link(&fake, true, dir.path());
    assert_eq!(l.focusing("w1:p1"), Some(true), "shown at once");
    let Done::FocusRead { announced, real } = done(&mut l).await else {
        panic!("not a focus read")
    };
    assert_eq!(
        (announced.as_str(), real.as_deref()),
        ("w1:p1", Some("w1:p7"))
    );
    assert_eq!(
        l.focus_read(&announced, real.clone()),
        Some(false),
        "herdr's real focus wins"
    );
    // a focus herdr reported meanwhile settles it: the late read changes nothing
    assert_eq!(l.focusing("w1:p1"), Some(true));
    l.herdr_focus("w1:p1");
    let Done::FocusRead { announced, real } = done(&mut l).await else {
        panic!("not a focus read")
    };
    assert_eq!(l.focus_read(&announced, real), None);
    assert_eq!(l.published.focused_pane.as_deref(), Some("w1:p1"));
    assert!(quiet(&mut l).await);
}

#[tokio::test(flavor = "current_thread")]
async fn a_moved_home_pane_moves_the_collectors_copy_too() {
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut l = link(&fake, true, dir.path());
    let cell: crate::snapshot::SelfPane =
        std::sync::Arc::new(std::sync::Mutex::new(Some("w1:p1".into())));
    l.collector_pane = Some(cell.clone());
    l.moved("w2:p9".into());
    assert_eq!(l.my_pane.as_deref(), Some("w2:p9"));
    assert_eq!(cell.lock().unwrap().as_deref(), Some("w2:p9"));
    assert_eq!(
        l.herdr_focus("w2:p9"),
        Some(true),
        "focus is judged against the new id"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn herdrs_focus_switches_the_docked_panels_look_too() {
    // test_focus_theme.py::test_herdrs_focus_events_switch_the_look_too: the open key focuses
    // panorama's pane through herdr, and no terminal focus report may come
    use crate::app::driver::Driver;
    use crate::app::model::App;
    use crate::ui::frame::Frame;
    use crate::ui::input::Painted;
    use panorama_core::view::theme::FIXED;
    let (fake, dir) = (FakeHerdr::start(), tempfile::tempdir().unwrap());
    let mut l = link(&fake, true, dir.path()); // panorama's own pane: w1:p1
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-design9.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut sw: panorama_core::model::Swarm = serde_json::from_value(v["swarm"].clone()).unwrap();
    sw.docked = Some("w1:p3".into());
    let t0 = std::time::Instant::now();
    let mut app = App::new(Some("w1:p3".into()), true, t0);
    app.apply(sw, Default::default(), t0);
    let mut d = Driver::new(app);
    let frame = Frame {
        now: v["now"].as_f64().unwrap(),
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let bg = |d: &mut Driver| {
        d.paint(46, 45, &frame, Painted::Other, t0)
            .unwrap()
            .cell(20, 30)
            .bg
    };
    let (focused, unfocused) = FIXED;
    assert!(d.focus(l.herdr_focus("w1:p3").unwrap())); // the agent got the keyboard
    assert!(!d.app.pane_focused && bg(&mut d) == Some(unfocused));
    assert!(d.focus(l.herdr_focus("w1:p1").unwrap())); // the open key: herdr focuses panorama
    assert!(d.app.pane_focused && bg(&mut d) == Some(focused));
}
