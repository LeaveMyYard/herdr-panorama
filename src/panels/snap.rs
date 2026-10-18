//! The part of herdr's `session.snapshot` the manager reads: every pane (its tab, workspace,
//! label, agent, focus and terminal), the focused tab and pane, and the pane each tab
//! remembers as focused (`layouts[].focused_pane_id`).

use herdr_agent_kit::herdr_api::Snapshot;
use serde_json::Value;

/// One pane, as the manager reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SnapPane {
    pub pane_id: String,
    pub tab_id: Option<String>,
    pub workspace_id: Option<String>,
    pub label: Option<String>,
    /// The agent kind herdr detected (`claude`), if any.
    pub agent: Option<String>,
    pub focused: bool,
    /// herdr's terminal id: a new one after a restore or a live handoff.
    pub terminal_id: Option<String>,
}

/// A snapshot, as the manager reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    pub panes: Vec<SnapPane>,
    pub focused_tab: Option<String>,
    pub focused_pane: Option<String>,
    /// The pane each tab remembers as focused.
    pub tab_focus: Vec<String>,
}

impl Snap {
    pub fn pane(&self, id: &str) -> Option<&SnapPane> {
        self.panes.iter().find(|p| p.pane_id == id)
    }

    /// The tab remembers `pane` as its focused one: the operator was last in it.
    pub fn tab_remembers(&self, pane: &str) -> bool {
        self.tab_focus.iter().any(|p| p == pane)
    }
}

impl From<Snapshot> for Snap {
    fn from(s: Snapshot) -> Self {
        let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
        let panes = s
            .panes
            .into_iter()
            .filter_map(|p| {
                Some(SnapPane {
                    pane_id: p.pane_id?.as_str().to_owned(),
                    tab_id: p.tab_id.map(|t| t.as_str().to_owned()),
                    workspace_id: p.workspace_id.map(|w| w.as_str().to_owned()),
                    label: p.label,
                    agent: p.agent,
                    focused: p.focused,
                    terminal_id: text(p.extra.get("terminal_id")),
                })
            })
            .collect();
        let tab_focus = s
            .layouts
            .iter()
            .filter_map(|l| text(l.get("focused_pane_id")))
            .collect();
        Self {
            panes,
            focused_tab: s.focused_tab_id.map(|t| t.as_str().to_owned()),
            focused_pane: s.focused_pane_id.map(|p| p.as_str().to_owned()),
            tab_focus,
        }
    }
}
