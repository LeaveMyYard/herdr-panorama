//! Working through a loss: read first, type once, then only read.
use super::{
    Resume,
    click::shape,
    decide,
    decide::{SETTLE_MS, SHELL_WAIT_MS, Verdict},
    hc::{Hc, Lookup, Sent},
    state::{Record, Stage},
    toggle::{Toggle, clicks, toggle},
};
use crate::{
    app_feed::fs::{Home, Result, digest},
    host::source::Linked,
};

const EARLY: &str = "a program ran before its shell was ready, so the first screen is unknown";
const CHANGED: &str = "the screen changed after Panorama first saw it, so someone may be using it";
const UNSEEN: &str = "Panorama didn't see this shell start, so its line is unproved";

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
                    self.click(home, &address, asked.get(&address).cloned(), allowed)?
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

    pub(super) fn judged(&self, record: &Record, rebooted: bool, address: &str) -> (Verdict, bool) {
        match self.hc.get(address) {
            Lookup::Gone => (Verdict::Dead("the terminal is gone"), false),
            Lookup::Unknown => (Verdict::Wait, false),
            Lookup::Found(term) => {
                let lost = term.pid.is_none() || term.reason.is_some();
                (decide::judge(record, &term, rebooted), lost)
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
        let rebooted = self.entry(address).rebooted;
        let (verdict, lost) = self.judged(&record, rebooted, address);
        match verdict {
            Verdict::Dead(reason) => {
                let entry = self.entry(address);
                entry.dead = true;
                entry.end(Stage::Skipped, reason);
                return Ok(());
            }
            Verdict::Skip(reason) => {
                self.entry(address).end(Stage::Skipped, reason);
                return Ok(());
            }
            // Before any prompt this may be the shell's own startup: keep watching, for the
            // button. After one, someone is using the terminal.
            Verdict::Busy(reason) => {
                let entry = self.entry(address);
                if entry.first.is_some() {
                    entry.end(Stage::Skipped, reason);
                } else {
                    entry.hold.get_or_insert_with(|| EARLY.into());
                }
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
            (None, true) => return self.fire(home, address, &first, false, allowed),
        }
        Ok(())
    }

    /// Spends the attempt on disk, then looks one last time, then types. `expected` is what
    /// the screen must still be: its digest, or for a click its digest ignoring digits.
    pub(super) fn fire(
        &mut self,
        home: &Home,
        address: &str,
        expected: &str,
        clicked: bool,
        allowed: &dyn Fn() -> bool,
    ) -> Result<()> {
        let now = (self.clock)();
        let entry = self.entry(address);
        let Some(command) = entry.record.command() else {
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
        if let Err(why) = self.last_look(home, address, expected, clicked, allowed) {
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
        expected: &str,
        clicked: bool,
        allowed: &dyn Fn() -> bool,
    ) -> std::result::Result<(), &'static str> {
        let loss = self.saved.as_ref().and_then(|s| s.loss.as_ref()).unwrap();
        let entry = &loss.entries[address];
        if !self.hc.quiet(address) {
            return Err("someone is typing in the terminal");
        }
        match self.judged(&entry.record, entry.rebooted, address).0 {
            Verdict::Ready => {}
            Verdict::Busy(reason) | Verdict::Skip(reason) | Verdict::Dead(reason) => {
                return Err(reason);
            }
            Verdict::Wait => return Err("the terminal could not be read"),
        }
        let seen = self.hc.screen(address).map(|text| {
            if clicked {
                shape(&text)
            } else {
                digest(text.as_bytes())
            }
        });
        if seen.as_deref() != Some(expected) {
            return Err(CHANGED);
        }
        if toggle(home) != Toggle::On || !allowed() {
            return Err("resume was turned off");
        }
        Ok(())
    }
}
