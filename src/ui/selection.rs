//! The selection both views keep (Python `SwarmApp.initial_selection`, `resolve`,
//! `selected_card`, `_update_dock`): by identity, never by row.

use panorama_core::model::{HerdrStatus, Status, Swarm};
use panorama_core::view::filter::visible_orchs;
use panorama_core::view::nav::{default_children, find_agent, find_orch, find_thread, nav};
use panorama_core::view::{NEW_ROW, View};
use std::time::Duration;

/// The selectable rows, in order: the orchestrators the filter shows, then `+ New agent`
/// (Python `render.nav(...)[0]`).
pub fn shown_ids(swarm: &Swarm, view: &View) -> Vec<String> {
    let mut ids: Vec<String> = visible_orchs(swarm, view)
        .iter()
        .map(|o| o.pane_id.clone())
        .collect();
    ids.push(NEW_ROW.to_owned());
    ids
}

/// The first selection: `focus_pane` when it is shown, else the first row; the keyboard in
/// the orchestrators' column; its remembered (else first) thread and agent.
pub fn initial(swarm: &Swarm, view: &mut View, focus_pane: Option<&str>) {
    let ids = shown_ids(swarm, view);
    view.orch = focus_pane
        .filter(|p| ids.iter().any(|i| i == p))
        .map(str::to_owned)
        .or_else(|| ids.first().cloned());
    view.focus = 0;
    default_children(swarm, view);
}

/// After a refresh or a filter change: every selection that still exists stays; one that
/// vanished is cleared. The orchestrator no longer shown, if any: where the arrows resume.
pub fn resolve(swarm: &Swarm, view: &mut View) -> Option<String> {
    let mut parked = None;
    if let Some(o) = &view.orch
        && !shown_ids(swarm, view).contains(o)
    {
        parked = view.orch.take();
    }
    let cols = nav(swarm, view);
    if view.thread.as_ref().is_some_and(|t| !cols[1].contains(t)) {
        view.thread = None;
    }
    let cols = nav(swarm, view);
    let in_tree = |a: &String| cols[2..].iter().any(|c| c.contains(a));
    if view.agent.as_ref().is_some_and(|a| !in_tree(a)) {
        view.agent = None;
    }
    parked
}

/// No key acts this long after what it would act on changed (Python `ARM_SECONDS`).
pub const ARM: Duration = Duration::from_millis(400);

/// What an action key would act on, as a comparable value (Python `selected_card`): a
/// refresh that changes it arms the keys again ([`ARM`]).
#[derive(Debug, PartialEq)]
pub struct SelectedCard {
    dock: bool,
    focus: usize,
    orch: Option<(String, HerdrStatus, Vec<i64>)>,
    inbox_ok: bool,
    thread: Option<String>,
    agent: Option<(String, Status, Vec<i64>, Option<String>)>,
}

/// The card `view` selects in `swarm`, in the docked panel (`dock`) or the graph.
pub fn selected_card(swarm: &Swarm, view: &View, dock: bool) -> SelectedCard {
    let o = find_orch(swarm, view.orch.as_deref());
    let t = find_thread(o, view.thread.as_deref());
    let a = find_agent(t, view.agent.as_deref());
    SelectedCard {
        dock,
        focus: view.focus,
        orch: o.map(|o| (o.pane_id.clone(), o.herdr_status.clone(), o.inbox.clone())),
        inbox_ok: swarm.inbox_ok,
        thread: t.map(|t| t.id.clone()),
        agent: a.map(|a| {
            (
                a.id.clone(),
                a.status.clone(),
                a.question_ids.clone(),
                a.transcript.clone(),
            )
        }),
    }
}

#[cfg(test)]
mod tests;
