//! The few hc calls agent resume makes. `send` is the only one that writes to a terminal.
use super::state::Config;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub core: String,
    pub uptime_ms: u64,
    pub holders: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foreground {
    pub pid: u32,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: String,
}

/// The fields of `hc get` that decide anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub live: bool,
    pub keep: bool,
    pub pid: Option<u32>,
    /// Set while the program is lost or ended; cleared once its shell runs.
    pub reason: Option<String>,
    pub created_ms: u64,
    /// What the terminal started. `exec` keeps the pid but changes the foreground's argv.
    pub argv: Vec<String>,
    /// The program's own report. A plain shell has none.
    pub reports: bool,
    pub foreground: Option<Foreground>,
}

impl Term {
    pub fn parse(v: &Value) -> Option<Self> {
        let number = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
        let words = |v: &Value| -> Option<Vec<String>> {
            let list = v.as_array()?.iter().map(|a| a.as_str().map(str::to_owned));
            list.collect()
        };
        let foreground = v.get("foreground").filter(|f| !f.is_null());
        Some(Self {
            live: v.get("state")?.as_str()? == "live",
            keep: v.get("keep").and_then(Value::as_bool).unwrap_or(false),
            pid: v.get("pid").and_then(number),
            reason: v.get("reason").and_then(Value::as_str).map(str::to_owned),
            created_ms: v.get("created_ms")?.as_u64()?,
            argv: words(v.get("argv")?)?,
            reports: v.get("status").is_some_and(|s| !s.is_null()),
            foreground: match foreground {
                None => None,
                Some(f) => Some(Foreground {
                    pid: number(f.get("pid")?)?,
                    name: f.get("name")?.as_str()?.to_owned(),
                    argv: words(f.get("argv")?)?,
                    cwd: f.get("cwd")?.as_str()?.to_owned(),
                }),
            },
        })
    }
}

/// From a viewer's first paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    /// Bracketed paste is on. bash, zsh and fish set it at their line editor and clear it
    /// for anything else, including a `read` in a startup file: it marks a real prompt.
    pub editing: bool,
    /// The cursor's screen row, from 1.
    pub row: usize,
}

impl Paint {
    /// The last cursor move in a paint is where the cursor ends up.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let text = String::from_utf8_lossy(bytes);
        let editing = text.contains("\x1b[?2004h");
        let row = text.rmatch_indices('H').find_map(|(end, _)| {
            let head = &text[..end];
            let start = head.rfind("\x1b[")? + 2;
            let (row, col) = head[start..].split_once(';')?;
            col.parse::<usize>().ok()?;
            row.parse().ok()
        })?;
        Some(Self { editing, row })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    Found(Term),
    Gone,
    /// Nothing can be concluded from this read.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    Accepted,
    /// hc's error kind, or `None` when the outcome is unknown. Either way it may have arrived.
    Failed(Option<String>),
}

pub trait Hc {
    fn status(&self) -> Option<Status>;
    fn get(&self, address: &str) -> Lookup;
    fn screen(&self, address: &str) -> Option<String>;
    /// True if nobody has typed here for hc's ~3 s. Types nothing.
    fn quiet(&self, address: &str) -> bool;
    /// What a viewer would be told. Changes nothing: `hc attach --observe`.
    fn paint(&self, address: &str) -> Option<Paint>;
    /// True if the shell has children, or that can't be read. In bash a startup child stays in
    /// the shell's process group, so `foreground` doesn't show it.
    fn children(&self, pid: u32) -> bool;
    /// Which config dir this process's record is in.
    fn config(&self, pid: u32) -> Config;
    /// True if a live process already holds this session.
    fn held(&self, session: &str) -> bool;
    fn send(&self, address: &str, text: &str) -> Sent;
}
