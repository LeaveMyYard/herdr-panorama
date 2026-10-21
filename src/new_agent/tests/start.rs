//! Starting the agent (Python `tests/test_home.py`'s new-agent cases, without the first
//! prompt: approved deviation 2026-10-17).

use crate::new_agent::KIND;
use crate::new_agent::ops::{NewTab, StartOps};
use crate::new_agent::start::new_agent;
use std::sync::Mutex;

/// An in-memory herdr: tabs made, agents, calls, and what fails.
#[derive(Default)]
struct World {
    calls: Mutex<Vec<String>>,
    agents: Mutex<Vec<String>>,
    fail: Vec<&'static str>,
    /// The new tab has no pane.
    paneless: bool,
}

impl World {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn log(&self, c: String) -> Result<(), String> {
        let failed = self.fail.iter().any(|f| c.starts_with(f));
        self.calls.lock().unwrap().push(c);
        if failed { Err("boom".into()) } else { Ok(()) }
    }
}

impl StartOps for World {
    async fn agent_exists(&self, name: &str) -> bool {
        self.agents.lock().unwrap().iter().any(|a| a == name)
    }
    async fn panel_layout(&self, label: &str, cwd: Option<&str>) -> Result<NewTab, String> {
        self.log(format!("layout {label} {}", cwd.unwrap_or("-")))?;
        Ok(NewTab {
            tab: "w1:t9".into(),
            pane: (!self.paneless).then(|| "w1:p91".into()),
            panel: Some("w1:p90".into()),
        })
    }
    async fn plain_tab(&self, label: &str, cwd: Option<&str>) -> Result<NewTab, String> {
        self.log(format!("tab {label} {}", cwd.unwrap_or("-")))?;
        Ok(NewTab {
            tab: "w1:t1".into(),
            pane: (!self.paneless).then(|| "w1:p1".into()),
            panel: None,
        })
    }
    async fn fit(&self, panel: &str, _tab: &str) {
        let _ = self.log(format!("fit {panel}"));
    }
    async fn start_agent(&self, name: &str, kind: &str, pane: &str) -> Result<(), String> {
        self.log(format!("start {name} {kind} {pane}"))?;
        self.agents.lock().unwrap().push(name.into());
        Ok(())
    }
    async fn close_tab(&self, tab: &str) -> Result<(), String> {
        self.log(format!("close {tab}"))
    }
    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        self.log(format!("focus_tab {tab}"))
    }
    async fn focus_pane(&self, pane: &str) -> Result<(), String> {
        self.log(format!("focus_pane {pane}"))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_agent_starts_in_its_own_tab_which_then_takes_the_focus() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let work = dir.path().to_str().unwrap();
    assert_eq!(
        new_agent(&w, "Billing API!", Some(work), KIND, false).await,
        Ok("billing-api".into())
    );
    assert_eq!(
        w.calls(),
        [
            format!("tab billing-api {work}"),
            "start billing-api claude w1:p1".into(),
            "focus_tab w1:t1".into()
        ]
    );
    assert!(
        !w.calls().iter().any(|c| c.starts_with("prompt")),
        "nothing is typed into the pane"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn per_tab_builds_the_tab_with_its_panel_on_the_left() {
    let w = World::default();
    assert_eq!(
        new_agent(&w, "fresh", None, KIND, true).await,
        Ok("fresh".into())
    );
    assert_eq!(
        w.calls(),
        [
            "layout fresh -",
            "fit w1:p90",
            "start fresh claude w1:p91",
            "focus_tab w1:t9",
            "focus_pane w1:p91"
        ]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_start_closes_its_tab_and_never_takes_the_focus() {
    let w = World {
        fail: vec!["start"],
        ..World::default()
    };
    let err = new_agent(&w, "search", None, KIND, false)
        .await
        .unwrap_err();
    assert!(err.starts_with("agent start failed"), "{err}");
    assert_eq!(w.calls().last().unwrap(), "close w1:t1");
    assert!(!w.calls().iter().any(|c| c.starts_with("focus")));
}

#[tokio::test(flavor = "current_thread")]
async fn names_directories_and_existing_agents_are_refused_before_anything_is_made() {
    let w = World::default();
    assert_eq!(
        new_agent(&w, "!!!", None, KIND, false).await,
        Err("the name needs at least one letter".into())
    );
    let dir = tempfile::tempdir().unwrap();
    let nope = dir.path().join("nope");
    let err = new_agent(&w, "docs", Some(nope.to_str().unwrap()), KIND, false)
        .await
        .unwrap_err();
    assert!(err.contains("directory"), "{err}");
    new_agent(&w, "docs", None, KIND, false).await.unwrap();
    let before = w.calls().len();
    assert_eq!(
        new_agent(&w, "Docs", None, KIND, false).await,
        Err("an agent named docs already exists".into())
    );
    assert_eq!(w.calls().len(), before, "an existing name: no tab");
}

#[tokio::test(flavor = "current_thread")]
async fn a_tab_that_could_not_be_made_or_has_no_pane() {
    let w = World {
        fail: vec!["tab"],
        ..World::default()
    };
    assert!(
        new_agent(&w, "a", None, KIND, false)
            .await
            .unwrap_err()
            .starts_with("tab create failed")
    );
    assert!(
        !w.calls()
            .iter()
            .any(|c| c.starts_with("close") || c.starts_with("start"))
    );
    let w = World {
        paneless: true,
        ..World::default()
    };
    assert_eq!(
        new_agent(&w, "a", None, KIND, true).await,
        Err("the new tab has no pane".into())
    );
    assert_eq!(w.calls().last().unwrap(), "close w1:t9");
}
