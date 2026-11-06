//! The Resume button: a first click shows the terminal, a second types if it hasn't changed.
use super::{Resume, decide::Verdict, hc::Hc, state::Look, toggle::Click};
use crate::app_feed::fs::{Home, Result, digest};

/// How long a click, and the view it asked for, stays good.
const CLICK_MS: u64 = 60_000;
const CLICK_SKEW_MS: u64 = 2_000;

/// The screen with digits blanked, as a digest: a clock in the prompt keeps ticking between
/// the two clicks. Digits typed onto the line change its length, so they still count.
pub(super) fn shape(text: &str) -> String {
    let blanked: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    digest(blanked.as_bytes())
}

fn last_line(text: &str) -> String {
    let line = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    line.trim_end().chars().take(120).collect()
}

impl<H: Hc> Resume<H> {
    /// A click, if it is for this row of this loss and recent. Stale, replayed and early
    /// clicks do nothing, so a click left in the store can't fire later with nobody there.
    pub(super) fn click(
        &mut self,
        home: &Home,
        address: &str,
        asked: Option<Click>,
        allowed: &dyn Fn() -> bool,
    ) -> Result<()> {
        let detected = self
            .saved
            .as_ref()
            .and_then(|s| s.loss.as_ref())
            .unwrap()
            .detected_ms;
        let now = (self.clock)();
        let entry = self.entry(address);
        if entry
            .look
            .as_ref()
            .is_some_and(|l| now.saturating_sub(l.at_ms) > CLICK_MS)
        {
            entry.look = None;
        }
        let Some(click) = asked.filter(|c| {
            c.loss == detected
                && c.at > detected
                && now.saturating_sub(c.at) <= CLICK_MS
                && c.at <= now.saturating_add(CLICK_SKEW_MS)
                && Some(c.at) != entry.click
        }) else {
            return Ok(());
        };
        entry.click = Some(click.at);
        if !entry.clickable() {
            return Ok(());
        }
        let Some(token) = click.token else {
            self.look(address);
            return Ok(());
        };
        let look = entry.look.take().filter(|l| l.token == token);
        let Some(look) = look else {
            entry.reason = Some("that view of the terminal has expired; look again".into());
            return Ok(());
        };
        entry.started = false;
        self.fire(home, address, &look.shape, true, allowed)
    }

    /// The first click: shows what is on the terminal, so a person can judge the line.
    fn look(&mut self, address: &str) {
        let record = self.entry(address).record.clone();
        let rebooted = self.entry(address).rebooted;
        let judged = self.judged(&record, rebooted, address).0;
        let now = (self.clock)();
        let screen = self.hc.screen(address);
        let quiet = self.hc.quiet(address);
        let entry = self.entry(address);
        entry.look = None;
        match (judged, screen) {
            (Verdict::Dead(reason), _) => {
                entry.dead = true;
                entry.reason = Some(reason.into());
            }
            (Verdict::Busy(reason) | Verdict::Skip(reason), _) => {
                entry.reason = Some(reason.into())
            }
            (Verdict::Ready, Some(text)) if quiet => {
                let shape = shape(&text);
                entry.look = Some(Look {
                    token: shape[..12].to_owned(),
                    line: last_line(&text),
                    shape,
                    at_ms: now,
                });
            }
            (Verdict::Ready, Some(_)) => {
                entry.reason = Some("someone is typing in the terminal".into())
            }
            _ => entry.reason = Some("the terminal could not be read".into()),
        }
    }
}
