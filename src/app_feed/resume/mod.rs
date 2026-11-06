//! Optional agent resume for herdr-app, off by default (docs/protocol/agent-resume.md).
//!
//! THIS IS A STOPGAP. It exists only until herdr-app or herdr-core resumes agents itself,
//! which upstream has tied to OSC 7501 reporting a resume reference (herdr-core#13). When
//! that lands, delete this module, its toggle and its report: nothing else depends on them.
//!
//! After a full core stop, crash or reboot, kept terminals come back as plain shells. While
//! the toggle is on, the writer records each terminal's proved session, and after a loss it
//! types the harness's resume command, once, into terminals that are provably idle shells.
mod attempt;
pub mod decide;
pub mod hc;
pub mod state;
pub mod store;
mod toggle;

use super::{
    fs::{Home, Result},
    receipt::Receipt,
};
use crate::host::source::Linked;
use decide::Change;
use hc::Hc;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::Value;
use state::{Entry, Saved, Stage};
use std::{
    collections::HashMap,
    fs::File,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use store::Store;
use toggle::{Toggle, toggle};

fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub struct Resume<H> {
    hc: H,
    id: String,
    store: Store,
    /// Per-core locks: the writer lock is per app home, and two homes can share a core.
    locks: PathBuf,
    held: Option<(String, File)>,
    saved: Option<Saved>,
    /// This writer has not looked yet: what the file says may be old.
    started: bool,
    settled: HashMap<String, (String, u64)>,
    clock: fn() -> u64,
}

impl<H: Hc> Resume<H> {
    pub fn new(hc: H, r: &Receipt, env: &PluginEnv) -> Self {
        let locks = env.xdg_state_home().join("herdr-panorama/resume");
        Self::with(hc, &r.id, &r.generation(), locks, wall_clock)
    }

    pub fn with(hc: H, id: &str, generation: &str, locks: PathBuf, clock: fn() -> u64) -> Self {
        Self {
            hc,
            id: id.into(),
            store: Store::new(generation),
            locks,
            held: None,
            saved: None,
            started: false,
            settled: HashMap::new(),
            clock,
        }
    }

    /// One look, after a collect. With the toggle off this reads one small file, makes no hc
    /// call, and removes any record so that nothing old can act when it is turned on again.
    pub fn tick(
        &mut self,
        home: &Home,
        linked: &[Linked],
        allowed: &dyn Fn() -> bool,
    ) -> Option<Value> {
        match toggle(home) {
            Toggle::Unknown => return None,
            Toggle::Off => {
                let _ = self.store.clear(home);
                self.saved = None;
                self.started = false;
                self.held = None;
                self.settled.clear();
                return None;
            }
            Toggle::On => {}
        }
        let error = self.step(home, linked, allowed).err();
        // An unreadable record stops the feature; the operator must see why.
        let unread = Saved::new(&self.id);
        let saved = self.saved.as_ref().unwrap_or(&unread);
        Some(saved.report(error.as_deref()))
    }

    /// There is a loss still being worked through: look again soon.
    pub fn busy(&self) -> bool {
        self.loss_entries().any(|(_, e)| e.open())
    }

    /// The writer is leaving on purpose, and the app is closed. Whatever happens next
    /// happens unwatched, so these records can no longer prove a session was live at a loss.
    pub fn close(&mut self, home: &Home) {
        if let Some(saved) = self.saved.as_mut() {
            saved.closed = true;
            let _ = self.store.save(home, saved);
        }
    }

    fn loss_entries(&self) -> impl Iterator<Item = (&String, &Entry)> {
        let loss = self.saved.as_ref().and_then(|s| s.loss.as_ref());
        loss.into_iter().flat_map(|l| l.entries.iter())
    }

    fn step(&mut self, home: &Home, linked: &[Linked], allowed: &dyn Fn() -> bool) -> Result<()> {
        if self.saved.is_none() {
            let mut saved = self
                .store
                .load(home, &self.id)?
                .unwrap_or(Saved::new(&self.id));
            // A send that a dead writer began is spent, whether or not it arrived.
            let sent = (self.clock)();
            for entry in saved.loss.iter_mut().flat_map(|l| l.entries.values_mut()) {
                if entry.stage == Stage::Sending {
                    entry.stage = Stage::Sent;
                    entry.sent_ms = Some(sent);
                }
            }
            self.saved = Some(saved);
        }
        let status = self.hc.status().ok_or("herdr-core status is unavailable")?;
        self.claim(&status.core)?;
        let now = (self.clock)();
        let saved = self.saved.as_mut().unwrap();
        let before = saved.clone();
        match decide::compare(saved, &status) {
            // After a reboot the app can get here first; the record must wait untouched.
            Change::NoHolderYet => return Ok(()),
            Change::Lost => decide::open(saved, &status, now),
            Change::Foreign => {
                saved.records.clear();
                saved.loss = None;
            }
            Change::Same => {
                // Nothing watched in between, so each session must be proved again.
                if saved.closed || !self.started {
                    saved.records.values_mut().for_each(|r| r.proved = false);
                }
                let hc = &self.hc;
                decide::track(saved, linked, &|address| hc.get(address), now);
            }
        }
        self.started = true;
        saved.closed = false;
        saved.core = Some(status.core);
        saved.holders = status.holders;
        if *saved != before {
            self.store.save(home, saved)?;
        }
        self.advance(home, linked, allowed)
    }

    /// One holder of this per-core lock acts; any other writer stays out for its whole life.
    fn claim(&mut self, core: &str) -> Result<()> {
        if self.held.as_ref().is_some_and(|(id, _)| id == core) {
            return Ok(());
        }
        if core.is_empty() || !core.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("herdr-core reported an unusable id".into());
        }
        std::fs::create_dir_all(&self.locks).map_err(|e| e.to_string())?;
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.locks.join(format!("{core}.lock")))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "another Panorama is resuming on this core")?;
        self.held = Some((core.to_owned(), file));
        Ok(())
    }

    fn entry(&mut self, address: &str) -> &mut Entry {
        let loss = self.saved.as_mut().and_then(|s| s.loss.as_mut());
        loss.and_then(|l| l.entries.get_mut(address))
            .expect("an entry listed this tick")
    }

    fn persist(&mut self, home: &Home) -> Result<()> {
        self.store.save(home, self.saved.as_ref().unwrap())
    }
}

#[cfg(test)]
mod tests;
