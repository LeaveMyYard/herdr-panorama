mod acts;
mod announce;
mod events;
mod filter;
mod focus;
mod panels;
mod verify;

use super::Link;
use crate::home::Places;
use crate::panel::actor::Place;
use herdr_agent_kit::herdr_api::fake::FakeHerdr;
use std::path::Path;
use std::time::Duration;

/// A link over `fake` without a core: the home view (`home`) or a popup, its pane `w1:p1`.
pub fn link(fake: &FakeHerdr, home: bool, dir: &Path) -> Link {
    let place = Place {
        plugin_id: "leavemyyard.herdr-panorama".into(),
        root: Some("/plugins/p".into()),
        new_agent_mark: dir.join("new-agent.mark"),
        home: "/home/op".into(),
    };
    let places = Places {
        record: dir.join("home.json"),
        lock: dir.join("home.lock"),
        wait: Duration::from_secs(2),
    };
    Link::bare(
        fake.client(),
        home,
        Some("w1:p1".into()),
        place,
        places,
        dir.join("panorama.log"),
    )
}

/// The next report, waiting at most 5 s.
pub async fn done(l: &mut Link) -> super::Done {
    for _ in 0..500 {
        if let Some(d) = l.try_done() {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no report from the herdr side")
}

/// Nothing reported within a while.
pub async fn quiet(l: &mut Link) -> bool {
    tokio::time::sleep(Duration::from_millis(200)).await;
    l.try_done().is_none()
}
