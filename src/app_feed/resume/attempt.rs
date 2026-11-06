//! Working through a loss: read first, type once, then only read.
use super::{
    Resume, decide,
    decide::{SETTLE_MS, SHELL_WAIT_MS, Verdict},
    hc::{Hc, Lookup, Sent},
    state::{Record, Stage},
    toggle::{Toggle, toggle},
};
use crate::{
    app_feed::fs::{Home, Result},
    host::source::Linked,
};
use std::collections::HashSet;

impl<H: Hc> Resume<H> {
    pub(super) fn advance(
        &mut self,
        home: &Home,
        linked: &[Linked],
        allowed: &dyn Fn() -> bool,
    ) -> Result<()> {
        let open: HashSet<String> = self
            .loss_entries()
            .filter(|(_, e)| e.open())
            .map(|(a, _)| a.clone())
            .collect();
        for address in open {
            let before = self.entry(&address).clone();
            if before.stage == Stage::Waiting {
                self.attempt(home, &address, allowed)?;
            } else {
                let lookup = self.hc.get(&address);
                let link = linked.iter().find(|l| l.address == address);
                let command = before.record.command().unwrap_or_default();
                let arrived = self
                    .hc
                    .screen(&address)
                    .is_some_and(|text| text.contains(&command));
                let now = (self.clock)();
                let session = link.map(|l| l.session.as_str());
                decide::confirm(self.entry(&address), &lookup, session, arrived, now);
            }
            if *self.entry(&address) != before {
                self.persist(home)?;
            }
        }
        Ok(())
    }

    fn judged(&self, record: &Record, address: &str) -> Verdict {
        match self.hc.get(address) {
            Lookup::Gone => Verdict::Skip("the terminal is gone"),
            Lookup::Unknown => Verdict::Wait,
            Lookup::Found(term) => decide::judge(record, &term),
        }
    }

    /// Reads first; types only into a settled bare shell, and spends the attempt before it does.
    fn attempt(&mut self, home: &Home, address: &str, allowed: &dyn Fn() -> bool) -> Result<()> {
        let now = (self.clock)();
        let detected = self
            .saved
            .as_ref()
            .and_then(|s| s.loss.as_ref())
            .unwrap()
            .detected_ms;
        if now.saturating_sub(detected) >= SHELL_WAIT_MS {
            self.entry(address)
                .end(Stage::Skipped, "no idle shell came back in time");
            return Ok(());
        }
        let record = self.entry(address).record.clone();
        match self.judged(&record, address) {
            Verdict::Skip(reason) => {
                self.entry(address).end(Stage::Skipped, reason);
                return Ok(());
            }
            Verdict::Wait => {
                self.settled.remove(address);
            }
            Verdict::Ready if self.settling(address, now) => {}
            Verdict::Ready => {
                // Last look, then the toggle again: it may have been turned off (herdr#4894).
                if self.judged(&record, address) != Verdict::Ready {
                    self.settled.remove(address);
                    return Ok(());
                }
                if toggle(home) != Toggle::On || !allowed() {
                    return Ok(());
                }
                return self.send(home, address);
            }
        }
        Ok(())
    }

    /// True until two reads of a drawn screen, far enough apart, are the same.
    fn settling(&mut self, address: &str, now: u64) -> bool {
        let Some(text) = self.hc.screen(address).filter(|t| !t.trim().is_empty()) else {
            return true;
        };
        match self.settled.get(address) {
            Some((same, at)) if *same == text => now.saturating_sub(*at) < SETTLE_MS,
            _ => {
                self.settled.insert(address.to_owned(), (text, now));
                true
            }
        }
    }

    fn send(&mut self, home: &Home, address: &str) -> Result<()> {
        let now = (self.clock)();
        let Some(command) = self.entry(address).record.command() else {
            self.entry(address).end(Stage::Skipped, "no resume command");
            return Ok(());
        };
        let entry = self.entry(address);
        entry.stage = Stage::Sending;
        entry.sent_ms = Some(now);
        // Not saved means not typed: a restart must find this attempt already spent.
        if let Err(error) = self.persist(home) {
            self.entry(address)
                .end(Stage::Skipped, "the attempt could not be saved");
            return Err(error);
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
}
