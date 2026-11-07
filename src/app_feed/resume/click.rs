//! The Resume button: a first click shows the terminal, a second types if it hasn't changed.
use super::{Resume, decide::Verdict, hc::Hc, state::Look, toggle::Click};
use crate::app_feed::fs::{Home, Result, digest};

/// How long a click, and the view it asked for, stays good.
const CLICK_MS: u64 = 60_000;
const CLICK_SKEW_MS: u64 = 2_000;
const LINE_LIMIT: usize = 300;
const ROW_LINES: usize = 3;
const TIP_LINES: usize = 12;
const NO_EDITOR: &str = "the shell's line editor isn't up: a read, or bracketed paste is off";
pub(super) const HELD: &str = "a live process already has this session";
const NO_CURSOR: &str =
    "the last line isn't the prompt, so Panorama can't show what Enter would submit";

fn blank(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect()
}

/// The screen with digits blanked, as a digest: a clock in the prompt keeps ticking between
/// the two clicks. Digits typed onto the line change its length, so they still count.
pub(super) fn shape(text: &str) -> String {
    digest(blank(text).as_bytes())
}

/// The last non-blank line, digits blanked, for comparing with a later screen.
pub(super) fn prompt_of(text: &str) -> Option<String> {
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    Some(blank(line.trim_end()))
}

/// The screen row of the last non-blank line, from 1.
pub(super) fn last_row(text: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |at| at + 1)
}

/// The end of a line, which is where anything typed is.
fn line_end(line: &str) -> String {
    let chars: Vec<char> = line.trim_end().chars().collect();
    chars[chars.len().saturating_sub(LINE_LIMIT)..]
        .iter()
        .collect()
}

fn tail(text: &str, count: usize) -> Vec<String> {
    let lines = text.lines().rev().filter(|l| !l.trim().is_empty());
    let mut kept: Vec<String> = lines.take(count).map(line_end).collect();
    kept.reverse();
    kept
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
        let paint = self.hc.paint(address);
        let free = !self.hc.held(&record.session);
        let entry = self.entry(address);
        let prompt = entry.prompt.clone();
        entry.look = None;
        if !free {
            entry.reason = Some(HELD.into());
            return;
        }
        match (judged, screen) {
            (Verdict::Dead(reason), _) => {
                entry.dead = true;
                entry.reason = Some(reason.into());
            }
            (Verdict::Busy(reason) | Verdict::Skip(reason), _) => {
                entry.reason = Some(reason.into())
            }
            (Verdict::Ready, Some(text)) if quiet => {
                // The line editor must be up, with the cursor on the last line, and that line
                // must start with the prompt. A listing or continuation fails one of these.
                let last = prompt_of(&text).unwrap_or_default();
                let prompt = prompt.or_else(|| Some(last.clone()));
                entry.prompt.clone_from(&prompt);
                if !paint.is_some_and(|p| p.editing) {
                    entry.reason = Some(NO_EDITOR.into());
                    return;
                }
                let on_prompt = paint.is_some_and(|p| p.row == last_row(&text));
                if !on_prompt
                    || !prompt.is_some_and(|p| last.starts_with(&p))
                    || last.ends_with('\\')
                {
                    entry.reason = Some(NO_CURSOR.into());
                    return;
                }
                let shape = shape(&text);
                entry.look = Some(Look {
                    token: shape[..12].to_owned(),
                    lines: tail(&text, ROW_LINES),
                    text: tail(&text, TIP_LINES).join("\n"),
                    shape,
                    at_ms: now,
                });
            }
            (Verdict::Ready, Some(_)) => {
                entry.reason = Some("someone is typing in the terminal".into())
            }
            _ => {
                entry.reason =
                    Some("the shell isn't free: a job it started is still running".into())
            }
        }
    }
}
