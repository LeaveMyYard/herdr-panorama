//! An in-memory herdr for the home module, as Python's stateful fake CLI of
//! `tests/test_home.py` behaves (and herdr 0.9.3 where it matters): tabs and panes in order,
//! the client's focus, every call made.

use crate::home::{At, HOME_LABEL, HomeOps, Places};
use crate::panels::shell::runs_panorama;
use crate::panels::snap::{Snap, SnapPane, SnapTab};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const PANORAMA: &[&str] = &["uv", "run", "--quiet", "--frozen", "--no-dev", "--project", "/plugins/panorama", "panorama", "tui"];
pub const VENV_PANORAMA: &[&str] = &["/plugins/panorama/.venv/bin/python3", "/plugins/panorama/.venv/bin/panorama", "tui"];

#[derive(Clone, Debug, Default)]
pub struct Pane {
    pub id: String,
    pub tab: String,
    pub ws: String,
    pub label: Option<String>,
    pub agent: Option<String>,
    pub name: Option<String>,
    /// The foreground's argv (`None`: `claude` for an agent, else `zsh`).
    pub argv: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub ws: String,
}

#[derive(Default)]
pub struct State {
    pub tabs: Vec<Tab>,
    pub panes: Vec<Pane>,
    pub n: u32,
    pub focus: Option<String>,
    pub calls: Vec<String>,
    /// Calls herdr fails ("snapshot", "process_info", "open_home", "move").
    pub fail: BTreeSet<&'static str>,
}

impl State {
    fn new_tab(&mut self, label: &str, ws: &str) -> (String, String) {
        self.n += 1;
        let (t, p) = (format!("{ws}:t{}", self.n), format!("{ws}:p{}", self.n));
        self.tabs.push(Tab { id: t.clone(), label: label.into(), ws: ws.into() });
        self.panes.push(Pane { id: p.clone(), tab: t.clone(), ws: ws.into(), ..Pane::default() });
        (t, p)
    }

    /// A tab with a Claude agent's pane (and plain panes beside it): (pane, tab).
    pub fn add_agent(&mut self, label: &str, extra: usize) -> (String, String) {
        let (t, p) = self.new_tab(label, "w1");
        self.panes.last_mut().unwrap().agent = Some("claude".into());
        for i in 0..extra {
            self.panes.push(Pane { id: format!("{p}x{i}"), tab: t.clone(), ws: "w1".into(), ..Pane::default() });
        }
        (p, t)
    }

    pub fn pane(&mut self, id: &str) -> &mut Pane {
        self.panes.iter_mut().find(|p| p.id == id).unwrap()
    }

    pub fn tab_of(&self, pane: &str) -> Option<String> {
        self.panes.iter().find(|p| p.id == pane).map(|p| p.tab.clone())
    }

    pub fn in_tab(&self, tab: &str) -> Vec<String> {
        self.panes.iter().filter(|p| p.tab == tab).map(|p| p.id.clone()).collect()
    }

    pub fn label(&self, tab: &str) -> Option<String> {
        self.tabs.iter().find(|t| t.id == tab).map(|t| t.label.clone())
    }

    /// Panorama's pane left beside `agent` in its tab by an older version's moving cockpit:
    /// first in the agent's tab, its own home tab gone.
    pub fn left_docked(&mut self, me: &str, agent: &str) {
        let old = self.tab_of(me).unwrap();
        let tab = self.tab_of(agent).unwrap();
        let i = self.panes.iter().position(|p| p.id == me).unwrap();
        let mut pane = self.panes.remove(i);
        pane.tab = tab;
        let j = self.panes.iter().position(|p| p.id == agent).unwrap();
        self.panes.insert(j, pane);
        self.tabs.retain(|t| t.id != old);
    }

    /// A pane the operator split off in the home tab (Claude named `billing` in it, or a shell).
    pub fn beside_home(&mut self, home: &str, agent: bool) -> (String, String) {
        let tab = self.tab_of(home).unwrap();
        let mut pane = Pane { id: "w1:p50".into(), tab: tab.clone(), ws: "w1".into(), ..Pane::default() };
        if agent {
            pane.agent = Some("claude".into());
            pane.name = Some("billing".into());
        }
        self.panes.push(pane);
        ("w1:p50".into(), tab)
    }

    pub fn drop_pane(&mut self, id: &str) {
        self.panes.retain(|p| p.id != id);
    }

    pub fn moves_of_others(&self, me: &str) -> Vec<String> {
        self.calls.iter().filter(|c| c.starts_with("move_pane ") && !c.starts_with(&format!("move_pane {me} "))).cloned().collect()
    }

    pub fn count(&self, prefix: &str) -> usize {
        self.calls.iter().filter(|c| c.starts_with(prefix)).count()
    }
}

#[derive(Clone, Default)]
pub struct World(pub Arc<Mutex<State>>);

impl World {
    pub fn s(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    pub fn calls(&self) -> Vec<String> {
        self.s().calls.clone()
    }
}

impl HomeOps for World {
    async fn snapshot(&self) -> Option<Snap> {
        let mut s = self.s();
        s.calls.push("snapshot".into());
        if s.fail.contains("snapshot") {
            return None;
        }
        let focus = s.focus.clone();
        let panes = s
            .panes
            .iter()
            .map(|p| SnapPane {
                pane_id: p.id.clone(),
                tab_id: Some(p.tab.clone()),
                workspace_id: Some(p.ws.clone()),
                label: p.label.clone(),
                agent: p.agent.clone(),
                focused: focus.as_ref() == Some(&p.id),
                name: p.name.clone(),
                ..SnapPane::default()
            })
            .collect();
        let tabs = s
            .tabs
            .iter()
            .map(|t| SnapTab { tab_id: t.id.clone(), label: Some(t.label.clone()), workspace_id: Some(t.ws.clone()) })
            .collect();
        let focused_tab = focus.as_ref().and_then(|f| s.tab_of(f));
        Some(Snap { panes, tabs, focused_pane: focus, focused_tab, ..Snap::default() })
    }

    async fn runs_tui(&self, pane: &str) -> Option<bool> {
        let mut s = self.s();
        s.calls.push(format!("process_info {pane}"));
        if s.fail.contains("process_info") {
            return None;
        }
        let p = s.panes.iter().find(|p| p.id == pane)?;
        let argv: Vec<String> = p.argv.clone().unwrap_or_else(|| {
            vec![if p.agent.is_some() { "claude".to_string() } else { "zsh".to_string() }]
        });
        let procs = if argv.is_empty() { json!([]) } else { json!([{"argv": argv, "name": argv[0]}]) };
        let info = serde_json::from_value(json!({"pane_id": pane, "foreground_processes": procs})).unwrap();
        runs_panorama(&info, "tui")
    }

    async fn open_home_pane(&self, focus: bool) -> Result<At, String> {
        let mut s = self.s();
        s.calls.push(format!("open_home {}", if focus { "focus" } else { "no-focus" }));
        if s.fail.contains("open_home") {
            return Err("boom".into());
        }
        let (t, p) = s.new_tab("Panorama", "w1");
        s.panes.last_mut().unwrap().argv = Some(PANORAMA.iter().map(|a| a.to_string()).collect());
        Ok(At { pane_id: p, tab_id: t })
    }

    async fn open_popup(&self) -> Result<(), String> {
        self.s().calls.push("open_popup".into());
        Ok(())
    }

    async fn rename_pane(&self, pane: &str, label: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("rename_pane {pane} {label}"));
        if let Some(p) = s.panes.iter_mut().find(|p| p.id == pane) {
            p.label = Some(label.into());
        }
        Ok(())
    }

    async fn rename_tab(&self, tab: &str, label: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("rename_tab {tab} {label}"));
        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
            t.label = label.into();
        }
        Ok(())
    }

    async fn move_tab_first(&self, tab: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("move_tab {tab} 0"));
        if let Some(i) = s.tabs.iter().position(|t| t.id == tab) {
            let t = s.tabs.remove(i);
            s.tabs.insert(0, t);
        }
        Ok(())
    }

    async fn move_to_new_tab(&self, pane: &str, workspace: &str, label: &str) -> Result<At, String> {
        let mut s = self.s();
        s.calls.push(format!("move_pane {pane} --new-tab {label} --no-focus {workspace}"));
        if s.fail.contains("move") {
            return Err("boom".into());
        }
        let old = s.tab_of(pane).ok_or("pane_not_found")?;
        s.n += 1;
        let t = format!("{workspace}:t{}", s.n);
        s.tabs.push(Tab { id: t.clone(), label: label.into(), ws: workspace.into() });
        let src_ws = s.pane(pane).ws.clone();
        s.pane(pane).tab = t.clone();
        let mut id = pane.to_string();
        if src_ws != workspace {
            // herdr 0.9.3: a pane moved into another workspace gets a new id
            s.n += 1;
            id = format!("{workspace}:p{}", s.n);
            let p = s.pane(pane);
            (p.id, p.ws) = (id.clone(), workspace.into());
        }
        if s.focus.as_deref() == Some(pane) {
            // with --no-focus the client stays on the old tab, on the pane left there
            s.focus = s.in_tab(&old).first().cloned().or(s.focus.clone());
        }
        if s.in_tab(&old).is_empty() {
            s.tabs.retain(|x| x.id != old); // a tab left empty is closed
        }
        Ok(At { pane_id: id, tab_id: t })
    }

    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("focus_tab {tab}"));
        let in_tab = s.in_tab(tab);
        if !in_tab.is_empty() && !s.focus.as_ref().is_some_and(|f| in_tab.contains(f)) {
            s.focus = in_tab.first().cloned();
        }
        Ok(())
    }
}

pub fn places(dir: &Path) -> Places {
    Places { record: dir.join("home-k.json"), lock: dir.join("home-k.lock"), wait: Duration::from_secs(2) }
}

pub fn label() -> String {
    HOME_LABEL.to_string()
}
