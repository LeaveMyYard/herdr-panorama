//! A bounded ledger of transient writer-start failures. Validation and provenance
//! failures never enter it: they refuse (before spawn) or disable (after spawn).
use super::{
    fs::{Home, Known, Result},
    json,
    receipt::{Receipt, SMALL},
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LEDGER: &str = "start.json";
pub const LEDGER_TEMP: &str = ".start.tmp";
/// The failure that would reach this count disables the generation instead.
pub const MAX_FAILURES: u32 = 8;
const FIRST_MS: u64 = 5_000;
pub const CAP_MS: u64 = 160_000;

/// Backoff after `failures` consecutive transient failures: 5 s doubling to 160 s.
pub fn backoff_ms(failures: u32) -> u64 {
    (FIRST_MS << failures.saturating_sub(1).min(5)).min(CAP_MS)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub v: u8,
    pub install_id: String,
    pub failures: u32,
    pub retry_at_ms: u64,
}
impl Ledger {
    pub fn read(home: &Home, r: &Receipt) -> Result<Option<(Self, Known)>> {
        let Some(known) = home.read(&r.path(LEDGER), SMALL, 0o600)? else {
            return Ok(None);
        };
        let v = json::parse(&known.bytes)?;
        json::keys(&v, &["v", "install_id", "failures", "retry_at_ms"])?;
        let l: Self = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if l.v != 1 || l.install_id != r.id || !(1..MAX_FAILURES).contains(&l.failures) {
            return Err("ambiguous start ledger; preserved".into());
        }
        Ok(Some((l, known)))
    }
    /// Remaining wait, if any. A deadline beyond the longest backoff (clock moved
    /// backwards) is treated as elapsed rather than deferring indefinitely.
    pub fn deferred_ms(&self) -> Option<u64> {
        let left = self.retry_at_ms.checked_sub(now_ms())?;
        (left > 0 && left <= CAP_MS).then_some(left)
    }
    /// Records failure `failures`; the caller must hold the writer lock or know no
    /// writer runs, because writers refuse a visible `.start.tmp`.
    pub fn record(
        home: &Home,
        r: &Receipt,
        failures: u32,
        expected: Option<&Known>,
    ) -> Result<u64> {
        let wait = backoff_ms(failures);
        let ledger = Self {
            v: 1,
            install_id: r.id.clone(),
            failures,
            retry_at_ms: now_ms().saturating_add(wait),
        };
        home.replace(
            &r.path(LEDGER),
            &r.path(LEDGER_TEMP),
            &serde_json::to_vec(&ledger).map_err(|e| e.to_string())?,
            expected,
        )?;
        Ok(wait)
    }
}
