//! An in-memory herdr as the manager sees it (Python `tests/test_panels.py::World`): tabs of
//! panes left to right, the focus, herdr's focus events, and every call made.

use crate::panels::snap::{Snap, SnapPane};
use crate::panels::{Clock, Focus, FocusLog, HomeRef, Manager, Ops, PANEL_LABEL, Places, Shell};
use panorama_core::model::Swarm;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct P {
    pub id: String,
    pub label: Option<String>,
    pub agent: Option<String>,
    pub terminal: Option<String>,
    pub focused: bool,
}

pub fn pane(id: &str) -> P {
    P {
        id: id.into(),
        ..P::default()
    }
}

pub fn agent(id: &str) -> P {
    P {
        agent: Some("claude".into()),
        ..pane(id)
    }
}

type Hook = Box<dyn FnMut(&mut State) + Send>;
/// (plain shell?, age, children)
type ShellLook = (Option<bool>, Option<f64>, Option<bool>);
type Runs = Box<dyn Fn(&State, &str) -> Option<bool> + Send>;
type Looks = Box<dyn FnMut(&mut State, &str) -> Shell + Send>;

pub struct State {
    pub tabs: Vec<(String, Vec<P>)>,
    pub focus: (String, String),
    pub calls: Vec<String>,
    pub n: u32,
    /// pane → "panel" for panes running our viewer.
    pub argv: BTreeMap<String, String>,
    /// pane → (plain shell?, age, children); default an idle shell herdr just started.
    pub shell: BTreeMap<String, ShellLook>,
    pub tab_focus: BTreeMap<String, String>,
    pub log: Option<Arc<FocusLog>>,
    pub on_swap: Option<Hook>,
    pub on_open: Option<Hook>,
    /// Overrides `runs_panel`; every ask is noted in `asked`.
    pub runs: Option<Runs>,
    pub asked: Vec<String>,
    /// Overrides `pane_shell` (the state is handed in for a click during the look).
    pub looks: Option<Looks>,
    /// Overrides `shell_state`.
    pub ages: Option<(Option<f64>, Option<bool>)>,
    /// herdr refuses to open a panel, with this.
    pub fail_open: Option<String>,
    /// Panes herdr refuses to close.
    pub fail_close: BTreeSet<String>,
    /// Plugin panes: `plugin pane close` refuses any other pane (herdr 0.9.3); a session
    /// restore brings panes back as plain panes.
    pub plugin: BTreeSet<String>,
}

impl State {
    /// A plugin pane running our viewer that this test put there (not opened by the manager).
    pub fn viewer(&mut self, id: &str) {
        self.argv.insert(id.into(), "panel".into());
        self.plugin.insert(id.into());
    }

    pub fn tab_of(&self, pane: &str) -> String {
        self.tabs
            .iter()
            .find(|(_, ps)| ps.iter().any(|p| p.id == pane))
            .unwrap()
            .0
            .clone()
    }

    pub fn tab(&mut self, tab: &str) -> &mut Vec<P> {
        &mut self.tabs.iter_mut().find(|(t, _)| t == tab).unwrap().1
    }

    pub fn pane(&mut self, id: &str) -> &mut P {
        self.tabs
            .iter_mut()
            .flat_map(|(_, ps)| ps.iter_mut())
            .find(|p| p.id == id)
            .unwrap()
    }

    pub fn order(&self, tab: &str) -> Vec<String> {
        self.tabs
            .iter()
            .find(|(t, _)| t == tab)
            .unwrap()
            .1
            .iter()
            .map(|p| p.id.clone())
            .collect()
    }

    pub fn record(&self, ev: Focus) {
        if let Some(log) = &self.log {
            log.record(ev);
        }
    }

    fn shell_pid(id: &str) -> i64 {
        7000 + i64::from_str_radix(id.rsplit('p').next().unwrap(), 16).unwrap()
    }

    /// herdr stopped and restored its session: every pane keeps its id and label but gets a new
    /// terminal running a plain login shell.
    pub fn restart_herdr(&mut self) {
        self.plugin.clear();
        let mut n = self.n;
        for (_, ps) in &mut self.tabs {
            for p in ps {
                n += 1;
                p.terminal = Some(format!("term_{n}"));
            }
        }
        self.n = n;
        self.argv.clear();
        self.calls.clear();
    }
}

#[derive(Clone)]
pub struct World(pub Arc<Mutex<State>>);

impl World {
    /// t1 holds the operator's plain pane (and panorama's home); t2..t4 an agent each.
    pub fn new() -> Self {
        let log = Arc::new(FocusLog::default());
        log.set_live(true);
        let tabs = vec![
            ("w1:t1".into(), vec![pane("w1:p1")]),
            ("w1:t2".into(), vec![agent("w1:p2")]),
            ("w1:t3".into(), vec![agent("w1:p3")]),
            ("w1:t4".into(), vec![agent("w1:p4")]),
        ];
        World(Arc::new(Mutex::new(State {
            tabs,
            focus: ("w1:t1".into(), "w1:p1".into()),
            calls: vec![],
            n: 100,
            argv: BTreeMap::new(),
            shell: BTreeMap::new(),
            tab_focus: BTreeMap::new(),
            log: Some(log),
            on_swap: None,
            on_open: None,
            runs: None,
            asked: vec![],
            looks: None,
            ages: None,
            fail_open: None,
            fail_close: BTreeSet::new(),
            plugin: BTreeSet::new(),
        })))
    }

    pub fn s(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    pub fn snap(&self) -> Snap {
        let s = self.s();
        let panes = s
            .tabs
            .iter()
            .flat_map(|(t, ps)| {
                ps.iter().map(move |p| SnapPane {
                    pane_id: p.id.clone(),
                    tab_id: Some(t.clone()),
                    workspace_id: Some("w1".into()),
                    label: p.label.clone(),
                    agent: p.agent.clone(),
                    focused: p.focused,
                    terminal_id: p.terminal.clone(),
                })
            })
            .collect();
        Snap {
            panes,
            focused_tab: Some(s.focus.0.clone()),
            focused_pane: Some(s.focus.1.clone()),
            tab_focus: s.tab_focus.values().cloned().collect(),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.s().calls.clone()
    }

    pub fn order(&self, tab: &str) -> Vec<String> {
        self.s().order(tab)
    }

    pub fn closes(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|c| c.starts_with("close"))
            .collect()
    }
}

impl Ops for World {
    async fn snapshot(&self) -> Option<Snap> {
        Some(self.snap())
    }

    async fn runs_panel(&self, pane: &str) -> Option<bool> {
        let mut s = self.s();
        s.asked.push(pane.into());
        match &s.runs {
            Some(f) => f(&s, pane),
            None => Some(s.argv.get(pane).is_some_and(|c| c == "panel")),
        }
    }

    async fn open_panel(&self, target: &str) -> Result<String, String> {
        let mut s = self.s();
        if let Some(e) = s.fail_open.clone() {
            s.calls.push(format!("open {target}"));
            return Err(e);
        }
        s.n += 1;
        let (n, id) = (s.n, format!("w1:p{}", s.n));
        let tab = s.tab_of(target);
        let ps = s.tab(&tab);
        let i = ps.iter().position(|p| p.id == target).unwrap();
        ps.insert(
            i + 1,
            P {
                label: Some(PANEL_LABEL.into()),
                terminal: Some(format!("term_{n}")),
                ..pane(&id)
            },
        );
        s.argv.insert(id.clone(), "panel".into());
        s.plugin.insert(id.clone());
        s.calls.push(format!("open {target}"));
        if let Some(mut hook) = s.on_open.take() {
            hook(&mut s);
            s.on_open = Some(hook);
        }
        Ok(id)
    }

    async fn swap(&self, source: &str, target: &str) -> Result<(), String> {
        let mut s = self.s();
        let tab = s.tab_of(source);
        if s.tab_of(target) != tab {
            return Err("not swapped: cross_tab".into()); // herdr 0.9.3 swaps within a tab only
        }
        let ps = s.tab(&tab);
        let i = ps.iter().position(|p| p.id == source).unwrap();
        let j = ps.iter().position(|p| p.id == target).unwrap();
        ps.swap(i, j);
        s.calls.push(format!("swap {source} {target}"));
        s.focus = (tab.clone(), source.into()); // herdr focuses the swap's source
        s.record(Focus::Tab(tab));
        s.record(Focus::Pane(source.into()));
        if let Some(mut hook) = s.on_swap.take() {
            hook(&mut s);
            s.on_swap = Some(hook);
        }
        Ok(())
    }

    async fn close_plugin_pane(&self, pane: &str) -> Result<(), String> {
        let mut s = self.s();
        if s.fail_close.contains(pane) {
            return Err("refused".into());
        }
        if !s.plugin.contains(pane) {
            return Err("plugin_pane_not_found".into()); // herdr 0.9.3: not a plugin pane
        }
        let tab = s.tab_of(pane);
        s.tab(&tab).retain(|p| p.id != pane);
        s.calls.push(format!("close {pane}"));
        Ok(())
    }

    async fn close_pane(&self, pane: &str) -> Result<(), String> {
        let mut s = self.s();
        let tab = s.tab_of(pane);
        s.tab(&tab).retain(|p| p.id != pane);
        s.calls.push(format!("close_pane {pane}"));
        Ok(())
    }

    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("focus_tab {tab}"));
        let first = s.order(tab)[0].clone();
        s.focus = (tab.into(), first);
        Ok(())
    }

    async fn focus_pane(&self, pane: &str) -> Result<(), String> {
        let mut s = self.s();
        s.calls.push(format!("focus_pane {pane}"));
        s.focus = (s.tab_of(pane), pane.into());
        Ok(())
    }

    async fn fit(&self, _panel: &str, _tab: &str) {}

    async fn pane_shell(&self, pane: &str) -> Shell {
        let mut s = self.s();
        if let Some(mut f) = s.looks.take() {
            let out = f(&mut s, pane);
            s.looks = Some(f);
            return out;
        }
        if s.argv.contains_key(pane) {
            return Shell::Busy;
        }
        match s.shell.get(pane).map_or(Some(true), |v| v.0) {
            None => Shell::Unknown,
            Some(true) => Shell::Plain(State::shell_pid(pane)),
            Some(false) => Shell::Busy,
        }
    }

    async fn shell_state(&self, shell: i64) -> (Option<f64>, Option<bool>) {
        let s = self.s();
        if let Some(a) = s.ages {
            return a;
        }
        let id = s
            .tabs
            .iter()
            .flat_map(|(_, ps)| ps)
            .find(|p| State::shell_pid(&p.id) == shell)
            .unwrap()
            .id
            .clone();
        s.shell
            .get(&id)
            .map_or((Some(3.0), Some(false)), |v| (v.1, v.2))
    }
}

/// The model's orchestrators, local.
pub fn orch(panes: &[&str]) -> Swarm {
    let os: Vec<_> = panes
        .iter()
        .map(|p| json!({"pane_id": p, "tab_label": ""}))
        .collect();
    serde_json::from_value(
        json!({"orchestrators": os, "generated_at": 0.0, "herdr_ok": true,
                                  "inbox_ok": true, "errors": []}),
    )
    .unwrap()
}

pub fn home() -> HomeRef {
    HomeRef {
        pane_id: "w1:p1".into(),
        tab_id: "w1:t1".into(),
    }
}

pub fn set(panes: &[&str]) -> BTreeSet<String> {
    panes.iter().map(|p| p.to_string()).collect()
}

pub fn places(dir: &Path) -> Places {
    Places {
        state_path: Some(dir.join("panels.json")),
        marker_dir: Some(dir.join("state")),
        home_lock: dir.join("home.lock"),
    }
}

pub fn manager(w: &World, dir: &Path) -> Manager<World> {
    manager_with(w, dir, Arc::new(Instant::now))
}

pub fn manager_with(w: &World, dir: &Path, clock: Clock) -> Manager<World> {
    let log = w.s().log.clone();
    Manager::new(w.clone(), log, places(dir), clock)
}

/// One pass, as the core runs it (no home lock in these tests).
pub async fn pass(m: &mut Manager<World>, w: &World, sw: &Swarm, verified: &[&str], start: bool) {
    let snap = w.snap();
    m.reconcile(snap, sw, Some(&home()), &set(verified), start, false)
        .await
        .unwrap();
}
