//! Opening and keeping the home tab (Python `home.ensure`, `_ensure_locked`,
//! `cockpit.undock`, `home.leave`). Every step runs under the home lock, re-reads herdr and
//! acts only on a pane verified to run `panorama tui`.

use super::find::{find, release_tab, scan, sweep};
use super::label::is_home_label;
use super::ops::HomeOps;
use super::{HOME_LABEL, Home, Places, record};
use std::time::Duration;

/// What `ensure` found or did, or why not.
pub type Ensured = Result<Home, String>;

/// The home lock for up to `wait`; `None` when it stayed busy.
async fn lock(
    places: &Places,
    wait: Duration,
) -> Option<herdr_agent_kit::plugin_env::lock::HeldLock> {
    let lock = herdr_agent_kit::plugin_env::lock::LockFile::new(places.lock.clone()).mode(0o644);
    tokio::task::spawn_blocking(move || {
        lock.exclusive_within(wait, Duration::from_millis(20), || true)
    })
    .await
    .ok()?
    .ok()?
}

/// Panorama's home pane, opened if missing (never a second one), focused when asked. One
/// verified panorama is kept (a second is left alone, never adopted or closed); a home an
/// older version left docked beside an agent goes back to a home tab of its own; a home tab
/// the operator split is left as it is.
pub async fn ensure(ops: &impl HomeOps, places: &Places, focus: bool) -> Ensured {
    let (home, last) = {
        let Some(_held) = lock(places, places.wait).await else {
            return Err("another panorama operation holds the home lock; try again".into());
        };
        ensure_locked(ops, places, focus).await?
    };
    if focus && !last {
        ops.focus_tab(&home.tab_id).await?; // outside the lock, as before
    }
    Ok(home)
}

/// `ensure` under the lock: (the home, whether nothing is left to do: a fresh tab opened
/// with the focus asked for).
async fn ensure_locked(
    ops: &impl HomeOps,
    places: &Places,
    focus: bool,
) -> Result<(Home, bool), String> {
    // unknown is not "missing": opening now could make a second home tab (a slow herdr)
    let Some(snap) = ops.snapshot().await else {
        return Err("herdr did not answer; not opening the home tab".into());
    };
    let (found, unknown) = scan(ops, &snap).await;
    sweep(ops, &snap).await; // no ◆ panorama tab without panorama in it (issue #9)
    let Some(mut home) = find(&found, None, places) else {
        if unknown {
            // a look-alike herdr could not check might be the live panorama
            return Err("could not verify panorama's pane; not opening the home tab".into());
        }
        let at = ops.open_home_pane(focus).await?;
        let home = Home {
            pane_id: at.pane_id,
            tab_id: at.tab_id,
        };
        let _ = record::save(&places.record, &home);
        let _ = ops.rename_pane(&home.pane_id, HOME_LABEL).await;
        let _ = ops.rename_tab(&home.tab_id, HOME_LABEL).await;
        let _ = ops.move_tab_first(&home.tab_id).await; // best effort: an older herdr lacks tab.move
        return Ok((home, true));
    };
    if snap.pane(&home.pane_id).and_then(|p| p.label.as_deref()) != Some(HOME_LABEL) {
        let _ = ops.rename_pane(&home.pane_id, HOME_LABEL).await; // an older home pane: once
    }
    if !is_home_label(snap.tab_label(&home.tab_id)) {
        // left docked beside an agent by an older version's moving cockpit: back home
        if let Some(moved) = undock(ops, places, &snap, &home).await {
            home = moved;
        }
    }
    Ok((home, false))
}

/// Panorama's pane back to a tab of its own in its workspace, labelled and first. The new
/// home, or `None` (it stays where it is).
async fn undock(
    ops: &impl HomeOps,
    places: &Places,
    snap: &crate::panels::snap::Snap,
    me: &Home,
) -> Option<Home> {
    let ws = snap.pane(&me.pane_id)?.workspace_id.clone()?;
    let at = ops
        .move_to_new_tab(&me.pane_id, &ws, HOME_LABEL)
        .await
        .ok()?;
    let moved = Home {
        pane_id: at.pane_id,
        tab_id: at.tab_id,
    };
    if !moved.tab_id.is_empty() {
        let _ = ops.move_tab_first(&moved.tab_id).await;
    }
    let _ = record::save(&places.record, &moved);
    Some(moved)
}

/// On quit: the home tab this pane is in loses its label first if other panes stay in it.
/// It never waits long for the lock (`QUIT_LOCK`): busy, the next `ensure`'s sweep covers it.
pub async fn leave(ops: &impl HomeOps, places: &Places, pane: &str) -> Result<(), String> {
    let Some(_held) = lock(places, super::QUIT_LOCK).await else {
        return Err("busy: relabel skipped (the next start sweeps it)".into());
    };
    let snap = ops.snapshot().await.ok_or("herdr did not answer")?;
    let Some(tab) = snap.pane(pane).and_then(|p| p.tab_id.clone()) else {
        return Ok(());
    };
    release_tab(ops, &snap, &tab, pane).await
}

/// The live home pane, preferring `prefer` (the calling instance's own pane).
pub async fn find_home(ops: &impl HomeOps, places: &Places, prefer: Option<&str>) -> Option<Home> {
    let snap = ops.snapshot().await?;
    let (found, _) = scan(ops, &snap).await;
    find(&found, prefer, places)
}
