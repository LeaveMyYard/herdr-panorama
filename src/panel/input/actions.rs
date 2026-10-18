//! What the action keys do (Python `SwarmApp.action_enter`, `action_goto`, `action_answer`,
//! `action_inbox`, `action_transcript`, `action_new_agent`, `action_filter`, `_seen`, with
//! `PanelApp`'s overrides): each returns the [`Effect`]s the actor carries out.

use super::effect::Effect;
use crate::panel::model::Panel;
use crate::ui::input::host::Host;
use panorama_core::view::nav::find_orch;
use std::time::Instant;

pub use crate::ui::input::actions::HELP;

impl Panel {
    /// g (and Enter on an orchestrator): to the selected orchestrator's tab. `switch`: from
    /// the docked list onto another orchestrator, the keyboard stays on a panel there.
    pub fn goto(&mut self, switch: bool, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !self.usable(&mut out) || !self.armed(now) {
            return out;
        }
        self.input.arrow_due = None; // a key or click acts now: no settled-arrow switch after it
        let Some(o) = self.selection().0.cloned() else {
            return out;
        };
        let ids: Vec<String> = o.agents().map(|a| a.id.clone()).collect();
        if let Some(machine) = o.machine.clone() {
            self.seen(ids, &mut out);
            let machine_id = o.machine_id.clone().unwrap_or_else(|| machine.clone());
            out.push(Effect::GotoRemote {
                machine_id,
                machine,
                pane: o.pane_id,
                label: o.tab_label,
            });
            return out;
        }
        let keep_focus = switch
            && self.input.dock
            && self.graph.swarm.docked.as_deref() != Some(o.pane_id.as_str())
            && self.input.settings.keep_focus;
        // its agents are seen once herdr confirms the pane is still there (the actor); while a
        // switch runs, that is all it does (`switch` makes no second one), as in Python
        out.push(Effect::Goto {
            pane: o.pane_id,
            label: o.tab_label,
            keep_focus,
            seen: ids,
        });
        out
    }

    /// n: + New agent, in its own herdr popup, in the selected orchestrator's directory (else
    /// any orchestrator's; the popup's own default without one).
    pub fn new_agent(&mut self) -> Vec<Effect> {
        let mut out = vec![];
        if !self.usable(&mut out) {
            return out;
        }
        let o = find_orch(&self.graph.swarm, self.graph.view.orch.as_deref());
        let cwd = o.and_then(|o| o.cwd.clone()).or_else(|| {
            self.graph
                .swarm
                .orchestrators
                .iter()
                .find_map(|o| o.cwd.clone())
        });
        out.push(Effect::NewAgent { cwd });
        out
    }
}

#[cfg(test)]
mod tests;
