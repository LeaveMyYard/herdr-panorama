//! The per-tab panels' manager, inside the core (Python `panels.py`; docs/DESIGN.md "Per-tab
//! panels" and "As built"). SAFETY: it opens, swaps and closes panes.
//!
//! Which tabs get a panel: tabs holding a local Claude orchestrator in the home pane's
//! workspace (not the home tab; with `cockpit_follow_tabs = "single"` only tabs where it is
//! the only pane).
//!
//! - **Placement** is [`Manager::place`] and nothing else: a split right of the agent without
//!   focus, a swap with the agent as its source, the width.
//! - **When:** at core start one batch for every tab without a panel, the operator put back
//!   where they were unless they moved meanwhile; later only in the tab the operator is in.
//! - **What is ours** (the manager closes nothing else): a pane running `panorama panel` that
//!   this core opened, or whose viewer said a verified hello. A look-alike still counts as the
//!   tab's panel (no second one is made), but is never closed.
//! - **Closed stays closed** (kept in the state dir) until the open key asks there again; a
//!   panel that dies on its own is made again, at most `CRASH_LIMIT` times per tab.
//! - **Orphans:** a tab whose orchestrator went away loses its panel after `REMOVE_AFTER`.
//! - **After a herdr restart** the restored panels are closed only when provably our unused
//!   panels ([`restored`]).
//!
//! The manager runs one pass at a time (`&mut self`), under the home lock when one is given.

pub mod fit;
pub mod focus_log;
mod keys;
pub mod ops;
mod place;
pub mod real;
mod reconcile;
mod record;
mod restored;
pub mod shell;
pub mod snap;
mod survey;
#[cfg(test)]
mod tests;

pub use focus_log::{Focus, FocusLog};
pub use keys::OpenKey;
pub use ops::{Ops, Shell};
pub use place::operator_moved;
pub use survey::{HomeRef, Survey};

use herdr_agent_kit::plugin_env::lock::LockFile;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// herdr's label for a panel pane (the manifest's title for the `panel` entrypoint).
pub const PANEL_LABEL: &str = "Panorama panel";
/// A tab without an orchestrator this long loses its panel.
pub const REMOVE_AFTER: Duration = Duration::from_secs(30);
/// A create in flight: herdr's snapshot may not show the pane yet.
pub const CREATING: Duration = Duration::from_secs(15);
/// How long a "not a panel" answer for a pane is believed.
pub const NOT_PANEL: Duration = Duration::from_secs(30);
/// How long a "runs our viewer" answer is believed (a pane never becomes another).
pub const PANEL: Duration = Duration::from_secs(30);
/// Panels of one tab that died on their own before the manager stops making them there.
pub const CRASH_LIMIT: u32 = 3;
/// How long a pass waits for the home lock.
pub const LOCK: Duration = Duration::from_secs(2);
/// A restored panel's shell is at most this old (seconds) when it is closed.
pub const RESTORED_SECONDS: f64 = 15.0;
/// Passes, from the first that sees a restore, on which restored panels are judged.
pub const RESTORE_PASSES: u32 = 3;
/// The settle wait for late focus events after the batch's swaps (`cockpit.SYNC_SECONDS`).
pub const SYNC: Duration = Duration::from_millis(300);

/// The manager's clock (monotonic; tests hold it).
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// The panel manager.
pub struct Manager<O: Ops> {
    pub(crate) ops: O,
    pub(crate) focus_log: Option<Arc<FocusLog>>,
    clock: Clock,
    state_path: Option<PathBuf>,
    /// The panels' crash markers live here (the viewers' state dir).
    marker_dir: Option<PathBuf>,
    home_lock: Option<LockFile>,
    /// Panel pane → tab, opened (or adopted) by this core.
    pub created: BTreeMap<String, String>,
    /// tab → until when a create counts as in flight.
    creating: BTreeMap<String, Instant>,
    /// Tabs whose new panel could not be swapped left yet.
    pub right: BTreeSet<String>,
    /// tab → since when it has no orchestrator.
    orphaned: BTreeMap<String, Instant>,
    /// The closed tabs and the panel record, as saved.
    pub(crate) saved: record::Saved,
    not_panel: BTreeMap<String, Instant>,
    panel: BTreeMap<String, Instant>,
    /// Our panels whose viewer said a verified hello.
    connected_once: BTreeSet<String>,
    /// tab → panels that died on their own.
    pub crashes: BTreeMap<String, u32>,
    /// Tabs given up on (`CRASH_LIMIT`), for this core's life (#80).
    pub broken: BTreeSet<String>,
    /// The start batch ran.
    pub started: bool,
    passes: u32,
    /// The pass that first saw a restore.
    restore_seen: Option<u32>,
    /// What to log (Python's log lines), for the app to take.
    notes: Vec<String>,
}

/// Where the manager keeps its state and finds the crash markers and the home lock.
#[derive(Clone, Debug, Default)]
pub struct Places {
    /// `panels-<session>.json` in the state dir.
    pub state_path: Option<PathBuf>,
    /// The viewers' state dir (`panel-crash-<pane>` markers).
    pub marker_dir: Option<PathBuf>,
    /// The home lock (`home-<session>.lock`): a pass holds it.
    pub home_lock: Option<PathBuf>,
}

impl<O: Ops> Manager<O> {
    pub fn new(ops: O, focus_log: Option<Arc<FocusLog>>, places: Places, clock: Clock) -> Self {
        let saved = record::load(places.state_path.as_deref());
        Self {
            ops,
            focus_log,
            clock,
            state_path: places.state_path,
            marker_dir: places.marker_dir,
            home_lock: places.home_lock.map(LockFile::new),
            created: BTreeMap::new(),
            creating: BTreeMap::new(),
            right: BTreeSet::new(),
            orphaned: BTreeMap::new(),
            saved,
            not_panel: BTreeMap::new(),
            panel: BTreeMap::new(),
            connected_once: BTreeSet::new(),
            crashes: BTreeMap::new(),
            broken: BTreeSet::new(),
            started: false,
            passes: 0,
            restore_seen: None,
            notes: vec![],
        }
    }

    /// The tabs whose panel the operator closed.
    pub fn closed(&self) -> &BTreeSet<String> {
        &self.saved.closed
    }

    /// The panels recorded for after a herdr restart: pane → (tab, terminal).
    pub fn record(&self) -> Vec<(String, String, String)> {
        self.saved
            .panes
            .iter()
            .map(|(p, r)| (p.clone(), r.tab.clone(), r.terminal.clone()))
            .collect()
    }

    /// What happened worth a log line since the last call.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    pub(crate) fn note(&mut self, line: String) {
        self.notes.push(line);
    }

    pub(crate) fn save(&mut self) {
        if let Some(path) = self.state_path.clone()
            && let Err(e) = record::save(&path, &self.saved)
        {
            self.note(e);
        }
    }

    /// Provably ours, so the manager may close it: this core opened it, or its viewer said a
    /// verified hello.
    pub fn ours(&self, pane: &str, verified: &BTreeSet<String>) -> bool {
        self.created.contains_key(pane) || verified.contains(pane)
    }

    /// tab → its panel, for the viewers' navigation (a connected one first).
    pub fn panels_map(&self, sv: &Survey, verified: &BTreeSet<String>) -> BTreeMap<String, String> {
        sv.panels
            .iter()
            .filter_map(|(t, ps)| {
                Some((
                    t.clone(),
                    ps.iter()
                        .find(|p| verified.contains(*p))
                        .or(ps.first())?
                        .clone(),
                ))
            })
            .collect()
    }

    /// The viewer left a crash marker; it is used up here.
    fn crashed(&self, pane: &str) -> bool {
        let Some(dir) = &self.marker_dir else {
            return false;
        };
        match std::fs::remove_file(crate::panel::crash::marker(dir, pane)) {
            Ok(()) => true,
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        }
    }

    /// The home lock for up to `LOCK`; `Err(())` when it stayed busy (or no lock is set: `Ok(None)`).
    async fn lock(&self) -> Result<Option<herdr_agent_kit::plugin_env::lock::HeldLock>, ()> {
        let Some(lock) = self.home_lock.clone() else {
            return Ok(None);
        };
        match tokio::task::spawn_blocking(move || {
            lock.exclusive_within(LOCK, Duration::from_millis(20), || true)
        })
        .await
        {
            Ok(Ok(Some(held))) => Ok(Some(held)),
            _ => Err(()),
        }
    }

    /// One pass under the home lock; `None` when the lock stayed busy.
    pub async fn reconcile(
        &mut self,
        snap: snap::Snap,
        swarm: &panorama_core::model::Swarm,
        home: Option<&HomeRef>,
        verified: &BTreeSet<String>,
        start: bool,
        single: bool,
    ) -> Option<Survey> {
        let _held = self.lock().await.ok()?;
        Some(
            self.reconcile_unlocked(snap, swarm, home, verified, start, single)
                .await,
        )
    }
}
