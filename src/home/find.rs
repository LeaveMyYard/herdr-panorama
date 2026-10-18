//! Finding panorama's home pane by content (Python `home._lookalikes`, `scan`, `find`,
//! `sweep`, `release_tab`). Looks are not proof: only a pane herdr says runs `panorama tui`
//! is ever adopted, renamed, moved or closed.

use super::label::{is_home_label, neutral_label};
use super::ops::HomeOps;
use super::{HOME_LABEL, Home, Places, record};
use crate::panels::snap::{Snap, SnapPane};

fn no_agent(p: &SnapPane) -> bool {
    p.agent.as_deref().is_none_or(str::is_empty)
}

/// Panes that look like the home pane, wherever they are: labelled `◆ panorama` with no
/// agent, or the only agent-less pane of a tab labelled `◆ panorama` (an older home pane).
pub fn lookalikes(snap: &Snap) -> Vec<SnapPane> {
    let mut out: Vec<SnapPane> = snap
        .panes
        .iter()
        .filter(|p| p.label.as_deref() == Some(HOME_LABEL) && no_agent(p))
        .cloned()
        .collect();
    for t in snap
        .tabs
        .iter()
        .filter(|t| is_home_label(t.label.as_deref().unwrap_or("")))
    {
        let mine: Vec<&SnapPane> = snap.in_tab(&t.tab_id).filter(|p| no_agent(p)).collect();
        if let [only] = mine[..]
            && !out.iter().any(|o| o.pane_id == only.pane_id)
        {
            out.push(only.clone());
        }
    }
    out
}

/// The look-alikes verified to run panorama's view, and whether one could not be checked.
pub async fn scan(ops: &impl HomeOps, snap: &Snap) -> (Vec<SnapPane>, bool) {
    let (mut verified, mut unknown) = (vec![], false);
    for p in lookalikes(snap) {
        match ops.runs_tui(&p.pane_id).await {
            Some(true) => verified.push(p),
            None => unknown = true,
            Some(false) => {}
        }
    }
    (verified, unknown)
}

/// The live home pane among `found`: `prefer` (the calling instance's own pane), else the
/// recorded one, else the first. The record follows it.
pub fn find(found: &[SnapPane], prefer: Option<&str>, places: &Places) -> Option<Home> {
    let known = record::load_pane(&places.record);
    let p = found
        .iter()
        .find(|c| Some(c.pane_id.as_str()) == prefer)
        .or_else(|| found.iter().find(|c| Some(&c.pane_id) == known.as_ref()))
        .or_else(|| found.first())?;
    let home = Home {
        pane_id: p.pane_id.clone(),
        tab_id: p.tab_id.clone().unwrap_or_default(),
    };
    if Some(&p.pane_id) != known.as_ref() {
        let _ = record::save(&places.record, &home);
    }
    Some(home)
}

/// Every `◆ panorama` tab that holds no panorama is renamed (after a crash, or a quit that
/// could not). A tab keeps the label while any agent-less pane in it is panorama or cannot
/// be checked.
pub async fn sweep(ops: &impl HomeOps, snap: &Snap) {
    for t in snap
        .tabs
        .iter()
        .filter(|t| is_home_label(t.label.as_deref().unwrap_or("")))
    {
        let panes: Vec<&SnapPane> = snap.in_tab(&t.tab_id).collect();
        let mut maybe = false;
        for p in panes.iter().filter(|p| no_agent(p)) {
            if ops.runs_tui(&p.pane_id).await != Some(false) {
                maybe = true;
                break;
            }
        }
        if !maybe {
            let _ = ops
                .rename_tab(&t.tab_id, &neutral_label(snap, &panes))
                .await;
        }
    }
}

/// Panorama's pane `leaving` is about to leave or close: a home tab that still holds other
/// panes loses the `◆ panorama` label first. Why not, if the rename failed.
pub async fn release_tab(
    ops: &impl HomeOps,
    snap: &Snap,
    tab: &str,
    leaving: &str,
) -> Result<(), String> {
    if !is_home_label(snap.tab_label(tab)) {
        return Ok(());
    }
    let others: Vec<&SnapPane> = snap.in_tab(tab).filter(|p| p.pane_id != leaving).collect();
    if others.is_empty() {
        return Ok(()); // panorama alone: the tab closes with its pane
    }
    ops.rename_tab(tab, &neutral_label(snap, &others)).await
}
