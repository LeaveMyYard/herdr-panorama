//! Working through a loss: read first, type once, then only read.
use super::{
    Resume, decide,
    decide::{SETTLE_MS, SHELL_WAIT_MS, Verdict},
    hc::{Hc, Lookup, Sent},
    state::{Record, Stage},
    toggle::{Toggle, clicks, toggle},
};
use crate::{
    app_feed::fs::{Home, Result, digest},
    host::source::Linked,
};

const CHANGED: &str = "the screen changed after Panorama first saw it, so someone may be using it";
const UNSEEN: &str =
    "Panorama first saw this shell after it had started, so it can't prove the line is empty";

impl<H: Hc> Resume<H> {
    /// `linked` is `None` between collects, when only the shells being watched are read.
    pub(super) fn advance(
        &mut self,
        home: &Home,
        linked: Option<&[Linked]>,
        allowed: &dyn Fn() -> bool,
    ) -> Result<()> {
        let addresses: Vec<String> = self.loss_entries().map(|(a, _)| a.clone()).collect();
        let asked = linked.map(|_| clicks(home)).unwrap_or_default();
        for address in addresses {
            let before = self.entry(&address).clone();
            match (before.stage, linked) {
                (Stage::Waiting, _) => self.attempt(home, &address, allowed)?,
                (Stage::Sending | Stage::Sent, Some(linked)) => self.confirm(&address, linked),
                (Stage::Skipped | Stage::Failed, Some(_)) => {
                    self.click(home, &address, asked.get(&address).copied(), allowed)?
                }
                _ => {}
            }
            if *self.entry(&address) != before {
                self.persist(home)?;
            }
        }
        Ok(())
    }

    fn confirm(&mut self, address: &str, linked: &[Linked]) {
        let entry = self.entry(address);
        let command = entry.record.command().unwrap_or_default();
        let lookup = self.hc.get(address);
        let arrived = self
            .hc
            .screen(address)
            .is_some_and(|text| text.contains(&command));
        let link = linked.iter().find(|l| l.address == address);
        let session = link.map(|l| l.session.as_str());
        let now = (self.clock)();
        decide::confirm(self.entry(address), &lookup, session, arrived, now);
    }

    fn judged(&self, record: &Record, address: &str) -> (Verdict, bool) {
        match self.hc.get(address) {
            Lookup::Gone => (Verdict::Skip("the terminal is gone"), false),
            Lookup::Unknown => (Verdict::Wait, false),
            Lookup::Found(term) => {
                let lost = term.pid.is_none() || term.reason.is_some();
                (decide::judge(record, &term), lost)
            }
        }
    }

    /// Reads only, until the shell's first screen has held for the whole settle time.
    fn attempt(&mut self, home: &Home, address: &str, allowed: &dyn Fn() -> bool) -> Result<()> {
        let detected = self
            .saved
            .as_ref()
            .and_then(|s| s.loss.as_ref())
            .unwrap()
            .detected_ms;
        let hold = self.entry(address).hold.clone();
        if (self.clock)().saturating_sub(detected) >= SHELL_WAIT_MS {
            let reason = hold.unwrap_or_else(|| "no idle shell came back in time".into());
            self.entry(address).end(Stage::Skipped, reason);
            return Ok(());
        }
        let record = self.entry(address).record.clone();
        let (verdict, lost) = self.judged(&record, address);
        match verdict {
            Verdict::Skip(reason) => {
                self.entry(address).end(Stage::Skipped, reason);
                return Ok(());
            }
            Verdict::Wait => {
                self.entry(address).witnessed |= lost;
                return Ok(());
            }
            Verdict::Ready => {}
        }
        let Some(screen) = self.hc.screen(address) else {
            return Ok(());
        };
        if screen.trim().is_empty() {
            self.entry(address).witnessed = true;
            return Ok(());
        }
        let now = (self.mono)();
        let seen = digest(screen.as_bytes());
        let first = self
            .entry(address)
            .first
            .get_or_insert(seen.clone())
            .clone();
        if first != seen {
            self.entry(address).end(Stage::Skipped, CHANGED);
            return Ok(());
        }
        let since = *self.settled.entry(address.to_owned()).or_insert(now);
        if now.saturating_sub(since) < SETTLE_MS {
            return Ok(());
        }
        let entry = self.entry(address);
        match (hold, entry.witnessed) {
            (Some(reason), _) => entry.end(Stage::Skipped, reason),
            (None, false) => entry.end(Stage::Skipped, UNSEEN),
            (None, true) => return self.fire(home, address, allowed),
        }
        Ok(())
    }

    /// A person asked. That replaces the automatic trigger and nothing else: the screen must
    /// still be the first one seen, and every terminal check runs again in `fire`.
    fn click(
        &mut self,
        home: &Home,
        address: &str,
        asked: Option<u64>,
        allowed: &dyn Fn() -> bool,
    ) -> Result<()> {
        let detected = self
            .saved
            .as_ref()
            .and_then(|s| s.loss.as_ref())
            .unwrap()
            .detected_ms;
        let entry = self.entry(address);
        let Some(at) = asked.filter(|at| *at > detected && Some(*at) != entry.click) else {
            return Ok(());
        };
        entry.click = Some(at);
        if !entry.clickable() {
            return Ok(());
        }
        entry.started = false;
        self.fire(home, address, allowed)
    }

    /// Spends the attempt on disk, then looks one last time, then types.
    fn fire(&mut self, home: &Home, address: &str, allowed: &dyn Fn() -> bool) -> Result<()> {
        let now = (self.clock)();
        let entry = self.entry(address);
        let (Some(command), Some(first)) = (entry.record.command(), entry.first.clone()) else {
            entry.end(Stage::Skipped, "no resume command");
            return Ok(());
        };
        let (stage, reason, sent) = (entry.stage, entry.reason.clone(), entry.sent_ms);
        entry.stage = Stage::Sending;
        entry.sent_ms = Some(now);
        entry.reason = None;
        // Not saved means not typed: a restart must find this attempt already spent.
        if let Err(error) = self.persist(home) {
            let entry = self.entry(address);
            (entry.stage, entry.reason, entry.sent_ms) = (stage, reason, sent);
            if stage == Stage::Waiting {
                entry.end(Stage::Skipped, "the attempt could not be saved");
            }
            return Err(error);
        }
        if let Err(why) = self.last_look(home, address, &first, allowed) {
            self.entry(address).end(Stage::Skipped, why);
            return Ok(());
        }
        let result = self.hc.send(address, &command);
        let entry = self.entry(address);
        entry.stage = Stage::Sent;
        if let Sent::Failed(kind) = result {
            let kind = kind.unwrap_or_else(|| "no answer".into());
            entry.reason = Some(format!("the send failed ({kind})"));
        }
        Ok(())
    }

    /// Everything that can be checked, as late as possible. The probe goes first because it
    /// is the slowest; the last two read no terminal, so they add nothing to the gap.
    fn last_look(
        &self,
        home: &Home,
        address: &str,
        first: &str,
        allowed: &dyn Fn() -> bool,
    ) -> std::result::Result<(), &'static str> {
        let record = &self
            .saved
            .as_ref()
            .and_then(|s| s.loss.as_ref())
            .unwrap()
            .entries[address]
            .record;
        if !self.hc.quiet(address) {
            return Err("someone is typing in the terminal");
        }
        match self.judged(record, address).0 {
            Verdict::Ready => {}
            Verdict::Skip(reason) => return Err(reason),
            Verdict::Wait => return Err("the terminal could not be read"),
        }
        if self
            .hc
            .screen(address)
            .map(|s| digest(s.as_bytes()))
            .as_deref()
            != Some(first)
        {
            return Err(CHANGED);
        }
        if toggle(home) != Toggle::On || !allowed() {
            return Err("resume was turned off");
        }
        Ok(())
    }
}
