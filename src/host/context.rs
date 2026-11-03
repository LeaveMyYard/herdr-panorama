//! Environment addresses are claims; process/session proof happens before enrichment.
use herdr_agent_kit::plugin_env::PluginEnv;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Core,
    Tui,
    Unknown,
}
impl Host {
    pub fn code(self) -> &'static str {
        match self {
            Self::Core => "herdr-core",
            Self::Tui => "herdr-tui",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub host: Host,
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
}
impl Context {
    pub fn from_env(env: &PluginEnv) -> Self {
        let terminal = env.var("HC_TERMINAL_ID").map(str::to_owned);
        let pane_id = env.pane_id().map(str::to_owned);
        let host = if terminal.is_some() {
            Host::Core
        } else if pane_id.is_some() {
            Host::Tui
        } else {
            Host::Unknown
        };
        Self {
            host,
            terminal,
            pane_id,
        }
    }
}

/// Core addresses are opaque, qualified identifiers, never names or shell fragments.
pub fn valid_address(address: &str) -> bool {
    let mut parts = address.split('/');
    let first = parts.next().unwrap_or("");
    let (machine, local) = match parts.next() {
        Some(local) => (Some(first), local),
        None => (None, first),
    };
    address.len() <= 90
        && parts.next().is_none()
        && machine.is_none_or(|m| {
            !m.is_empty()
                && m.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
        && local.strip_prefix('t').is_some_and(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
        })
}

pub fn pane_key(terminal: &str) -> String {
    format!("core:{terminal}")
}

#[cfg(test)]
mod tests;
