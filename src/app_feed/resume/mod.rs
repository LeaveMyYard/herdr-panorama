//! Optional agent resume for herdr-app, off by default (docs/protocol/agent-resume.md).
//!
//! THIS IS A STOPGAP. It exists only until herdr-app or herdr-core resumes agents itself,
//! which upstream has tied to OSC 7501 reporting a resume reference (herdr-core#13). When
//! that lands, delete this module, its toggle and its report: nothing else depends on them.
//!
//! After a full core stop, crash or reboot, kept terminals come back as plain shells. While
//! the toggle is on, the writer records each terminal's proved session. After a reboot it
//! types the harness's resume command, once, into a shell it saw come back and stay untouched.
//! Everything else gets a Resume button with a confirm step.
mod attempt;
mod click;
pub mod core_hc;
pub mod decide;
pub mod hc;
mod launch;
pub mod outcome;
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
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    sync::OnceLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use store::Store;
use toggle::{Toggle, toggle};

fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn monotonic() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// The kernel's id for this boot. Elsewhere there is none, so a loss always needs a click.
fn boot_id() -> Option<String> {
    let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    Some(id.trim().to_owned()).filter(|id| !id.is_empty())
}

/// The fixed clocks and ids `Resume` reads; tests supply their own.
#[derive(Clone, Copy)]
pub struct Sources {
    pub clock: fn() -> u64,
    pub mono: fn() -> u64,
    pub boot: fn() -> Option<String>,
}

const SEEN_EVERY_MS: u64 = 60_000;

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
    /// When each shell's first screen was seen, on the monotonic clock.
    settled: HashMap<String, u64>,
    /// A restarted core has no holder yet: look again soon.
    no_holder: bool,
    /// When this writer last typed by itself, on the monotonic clock.
    last_fire: Option<u64>,
    clock: fn() -> u64,
    mono: fn() -> u64,
    boot: fn() -> Option<String>,
}

impl<H: Hc> Resume<H> {
    pub fn new(hc: H, r: &Receipt, env: &PluginEnv) -> Self {
        let locks = env.xdg_state_home().join("herdr-panorama/resume");
        let sources = Sources {
            clock: wall_clock,
            mono: monotonic,
            boot: boot_id,
        };
        Self::with(hc, &r.id, &r.generation(), locks, sources)
    }

    pub fn with(hc: H, id: &str, generation: &str, locks: PathBuf, sources: Sources) -> Self {
        Self {
            hc,
            id: id.into(),
            store: Store::new(generation),
            locks,
            held: None,
            saved: None,
            started: false,
            settled: HashMap::new(),
            no_holder: false,
            last_fire: None,
            clock: sources.clock,
            mono: sources.mono,
            boot: sources.boot,
        }
    }

    /// One look, after a collect. With the toggle off this makes no hc call, and removes any
    /// record so that nothing old can act when it is turned on again.
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
        self.no_holder || self.loss_entries().any(|(_, e)| e.open())
    }

    /// Shells are being watched for their first screen: the sooner the first read, the less
    /// time anyone had to type before it. Makes no collect.
    pub fn watching(&self) -> bool {
        self.loss_entries().any(|(_, e)| e.stage == Stage::Waiting)
    }

    /// Between collects, while `watching`.
    pub fn watch(&mut self, home: &Home, allowed: &dyn Fn() -> bool) {
        if toggle(home) == Toggle::On {
            let _ = self.advance(home, None, allowed);
        }
    }

    /// The writer is leaving on purpose, and the app is closed. Whatever happens next
    /// happens unwatched, so these records can no longer prove a session was live at a loss.
    pub fn close(&mut self, home: &Home) {
        if toggle(home) == Toggle::Off {
            let _ = self.store.clear(home);
        } else if let Some(saved) = self.saved.as_mut() {
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
                // A view from a writer that stopped is nobody's to confirm.
                entry.look = None;
            }
            // Saved at once: the file must not keep a stale view's screen text.
            if saved.loss.is_some() {
                self.store.save(home, &saved)?;
            }
            self.saved = Some(saved);
        }
        let status = self.hc.status().ok_or("herdr-core status is unavailable")?;
        self.claim(&status.core)?;
        let now = (self.clock)();
        let boot = (self.boot)();
        let saved = self.saved.as_mut().unwrap();
        let before = saved.clone();
        let change = decide::compare(saved, &status);
        self.no_holder = change == Change::NoHolderYet;
        match change {
            // After a reboot the app can get here first; the record must wait untouched.
            Change::NoHolderYet => return Ok(()),
            Change::Lost => decide::open(saved, &status, boot.as_deref(), now),
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
                decide::track(
                    saved,
                    linked,
                    &|address| hc.get(address),
                    &|pid| hc.config(pid),
                    now,
                );
            }
        }
        self.started = true;
        saved.closed = false;
        if now.saturating_sub(saved.seen_ms) >= SEEN_EVERY_MS {
            saved.seen_ms = now;
        }
        saved.core = Some(status.core);
        saved.holders = status.holders;
        // A failed read must not wipe the id: the next loss would have nothing to compare.
        saved.boot = boot.or(saved.boot.take());
        if *saved != before {
            self.store.save(home, saved)?;
        }
        self.advance(home, Some(linked), allowed)
    }

    /// One holder of this per-core lock acts; any other writer stays out for its whole life.
    fn claim(&mut self, core: &str) -> Result<()> {
        if self.held.as_ref().is_some_and(|(id, _)| id == core) {
            return Ok(());
        }
        if core.is_empty() || !core.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("herdr-core reported an unusable id".into());
        }
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.locks)
            .map_err(|e| e.to_string())?;
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
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
