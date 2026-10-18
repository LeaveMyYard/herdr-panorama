//! The app's state (Python `SwarmApp.apply`, `failed`, `initial_selection`, `resolve`,
//! `_update_dock`, `armed`, and what `repaint` derives): the collector's refreshes change it,
//! the actor paints it. No IO: every method takes the time.
//!
//! The moving cockpit is dropped (Q187): no dock or follow runs here, so nothing holds the
//! docked panel while a move resizes the pane, and no dock hint outlives a collect.

use crate::ui::layout;
use crate::ui::selection::{self, ARM, SelectedCard};
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, Screen, View};
use std::time::Instant;

/// A refresh error is cut to this many characters (Python `message[:200]`).
const ERROR_CHARS: usize = 200;

/// One `panorama tui` view.
#[derive(Debug)]
pub struct App {
    /// The last good swarm (an empty one before the first refresh).
    pub swarm: Swarm,
    /// What is drawn: the selection, the filter, the unseen marks, the scroll.
    pub view: View,
    /// A refresh has arrived.
    pub loaded: bool,
    /// The pane the app was opened from: selected first when it is shown.
    pub focus_pane: Option<String>,
    /// The home tab's view (it stays open after an action) rather than a popup.
    pub home_mode: bool,
    /// Why the last refresh failed (the last good model stays).
    pub error: Option<String>,
    /// Keys act from then on.
    pub armed_at: Instant,
    /// The docked panel, as decided on the last refresh or paint.
    pub dock: bool,
    /// The pane's width at the last paint (0 before the first).
    pub width: i32,
    /// Where the arrows resume from while no orchestrator is selected.
    parked: Option<String>,
}

impl App {
    /// An app opened from `focus_pane`, at `now`.
    pub fn new(focus_pane: Option<String>, home_mode: bool, now: Instant) -> Self {
        Self {
            swarm: Swarm::default(),
            view: View::default(),
            loaded: false,
            focus_pane,
            home_mode,
            error: None,
            armed_at: now,
            dock: false,
            width: 0,
            parked: None,
        }
    }

    /// A refresh: the swarm and its unseen marks, at `now`. The first selects the focus pane
    /// (else the first row); later ones keep every selection still shown. The keys wait
    /// ([`ARM`]) after the first one and whenever what they would act on changed.
    pub fn apply(&mut self, swarm: Swarm, marks: Marks, now: Instant) {
        self.error = None;
        let before = self.selected_card();
        self.swarm = swarm;
        self.view.unseen = marks;
        if !self.loaded {
            self.loaded = true;
            selection::initial(&self.swarm, &mut self.view, self.focus_pane.as_deref());
            self.armed_at = now + ARM;
            return;
        }
        if let Some(p) = selection::resolve(&self.swarm, &mut self.view) {
            self.parked = Some(p);
        }
        self.update_dock(); // before the comparison: switching layouts changes what keys do
        if self.selected_card() != before {
            self.armed_at = now + ARM;
        }
    }

    /// A refresh failed: the last good model stays, the header says why.
    pub fn failed(&mut self, message: &str) {
        self.error = Some(message.chars().take(ERROR_CHARS).collect());
    }

    /// Why the model may be out of date: the last refresh's error, else the collector's.
    pub fn stale(&self) -> Option<&str> {
        self.error
            .as_deref()
            .or(self.swarm.errors.first().map(String::as_str))
    }

    /// Whether the keys act at `now`.
    pub fn armed(&self, now: Instant) -> bool {
        now >= self.armed_at
    }

    /// Docked next to an agent and narrower than the wide layout: the docked panel, its list
    /// one column (the keyboard on it).
    pub fn update_dock(&mut self) {
        self.dock = layout::docked(&self.swarm, self.width);
        if self.dock {
            self.view.focus = 0;
        }
    }

    /// The rule's `z panel` hint: the home tab's view, docked next to an agent (`z` zooms
    /// this pane to the full graph; the per-tab panel, which cannot, is not this app).
    pub fn zoom_hint(&self) -> bool {
        self.home_mode && self.swarm.docked.is_some()
    }

    /// Before a paint of `w` × `h`: the layout for that width, the zoom hint. Whether to
    /// paint: a pane with no size yet is not painted, and keeps its last layout (Python's
    /// `repaint` returns first).
    pub fn before_paint(&mut self, w: i32, h: i32) -> bool {
        if w <= 0 || h <= 0 {
            return false;
        }
        self.width = w;
        self.update_dock();
        self.view.zoom_hint = self.zoom_hint();
        true
    }

    /// After a paint: each column's scroll offset, kept for the next one (Python's `scroll`
    /// wrote them into the view as it rendered).
    pub fn painted(&mut self, s: &Screen) {
        for col in &s.columns {
            self.view.offsets.insert(col.column, col.offset);
        }
    }

    /// What an action key would act on.
    pub fn selected_card(&self) -> SelectedCard {
        selection::selected_card(&self.swarm, &self.view, self.dock)
    }

    /// Where the arrows resume from while no orchestrator is selected.
    pub fn parked(&self) -> Option<&str> {
        self.parked.as_deref()
    }
}

#[cfg(test)]
mod tests;
