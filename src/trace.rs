//! `PANORAMA_TRACE=<file>` (Python `herdr.trace`): one JSON line per herdr call, appended, as
//! Python writes it: `{"at": <end, s, 3 places>, "op": "socket <method>" | "cli <args>",
//! "ms": <duration, 1 place>}`. Off unless the variable names a file; a write that fails is
//! dropped, never the call's. The kit's call observer hears the calls; every herdr client
//! panorama makes comes from [`client`] or [`blocking_client`], so each is traced.

use crate::snapshot::pyjson::{J, dumps_line_ascii};
use herdr_agent_kit::herdr_api::{CallTrace, Herdr, Transport, blocking};
use std::io::Write;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// The variable naming the trace file.
pub const VAR: &str = "PANORAMA_TRACE";

/// Python's `round(x, places)`: the decimal nearest the exact value, ties to even.
fn round(x: f64, places: usize) -> f64 {
    format!("{x:.places$}").parse().unwrap_or(x)
}

/// The line Python writes for `op` ended at `at` (POSIX seconds) after `ms` milliseconds.
pub fn line(op: &str, at: f64, ms: f64) -> String {
    dumps_line_ascii(&J::obj([
        ("at", J::Float(round(at, 3))),
        ("op", J::str(op)),
        ("ms", J::Float(round(ms, 1))),
    ]))
}

/// Python's op for a call: `socket <method>` or `cli <first two arguments>`.
pub fn op(t: &CallTrace) -> String {
    let kind = if t.transport == Transport::Cli {
        "cli"
    } else {
        "socket"
    };
    format!("{kind} {}", t.op)
}

/// Appends each call to `path`.
pub fn writer(path: PathBuf) -> impl Fn(&CallTrace) + Send + Sync + 'static {
    move |t: &CallTrace| {
        let end = t.started + t.took;
        let at = end
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        let text = line(&op(t), at, t.took.as_secs_f64() * 1000.0) + "\n";
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

/// The trace file `var` names, if any (an empty value is none).
fn file(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    var(VAR).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// panorama's async herdr client of this process's server, traced when asked.
pub fn client() -> Herdr {
    client_from(|n| std::env::var_os(n))
}

/// The same, blocking.
pub fn blocking_client() -> blocking::Herdr {
    client_from(|n| std::env::var_os(n)).blocking()
}

/// [`client`] over `var` instead of the environment (the server, the binary, the trace file).
pub fn client_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Herdr {
    let trace = file(&var);
    let h = Herdr::resolve(var).with_client("panorama");
    match trace {
        Some(path) => h.with_observer(writer(path)),
        None => h,
    }
}

#[cfg(test)]
mod tests;
