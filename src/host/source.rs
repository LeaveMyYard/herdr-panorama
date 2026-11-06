//! The host list behind a small trait; the collector still receives its established model.
use super::{
    Core, Host,
    context::{pane_key, valid_address},
    link::{Link, Linker, unique},
    records,
};
use herdr_agent_kit::{
    agent_procs::SystemProcesses, herdr_api::blocking::Herdr, plugin_env::PluginEnv,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

pub struct Observation {
    pub host: Host,
    pub snapshot: Option<Value>,
    pub terminals: HashMap<String, String>,
    pub errors: Vec<String>,
    /// Terminals whose foreground agent is proved to run a session, this collect.
    pub linked: Vec<Linked>,
}

/// A link that passed session-linking.md §4, with the `hc get` object it was proved against.
#[derive(Clone, Debug, PartialEq)]
pub struct Linked {
    pub address: String,
    pub harness: String,
    pub session: String,
    pub terminal: Value,
    /// The process uses the config dir a new shell would, as far as can be read.
    pub default_config: bool,
}

pub trait SnapshotSource: Send {
    fn read(&mut self) -> Observation;
}

pub struct Tui(pub Herdr);
impl SnapshotSource for Tui {
    fn read(&mut self) -> Observation {
        Observation {
            host: Host::Tui,
            snapshot: crate::snapshot::herdr_snapshot(&self.0),
            terminals: HashMap::new(),
            errors: Vec::new(),
            linked: Vec::new(),
        }
    }
}

pub struct CoreSource {
    pub core: Core,
    pub env: PluginEnv,
    pub hooks_dir: PathBuf,
    /// Process-derived linking (session-linking.md).
    pub link: Linker,
}
impl SnapshotSource for CoreSource {
    fn read(&mut self) -> Observation {
        let mut observation = Observation {
            host: Host::Core,
            snapshot: None,
            terminals: HashMap::new(),
            errors: Vec::new(),
            linked: Vec::new(),
        };
        match self.core.list().and_then(normalize) {
            Err(error) => observation.errors.push(format!("herdr-core: {error}")),
            Ok(mut rows) => {
                let hooks = records::load(&self.hooks_dir);
                if !hooks.complete {
                    observation
                        .errors
                        .push("hooks: not every hook file was read".to_owned());
                }
                let table = SystemProcesses::new();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                let mut links = HashMap::new();
                let mut read = HashMap::new();
                for row in &rows {
                    let address = row["terminal"].as_str().unwrap();
                    let key = pane_key(address);
                    observation
                        .terminals
                        .insert(key.clone(), address.to_owned());
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    // Remote PIDs cannot prove ownership on this machine.
                    let terminal = (!remaining.is_zero() && !address.contains('/'))
                        .then(|| self.core.with_timeout(remaining).terminal(address).ok())
                        .flatten();
                    let process = terminal
                        .as_ref()
                        .and_then(|t| super::core::process(t, address));
                    let link = process.as_ref().map_or((None, Link::Unavailable), |info| {
                        self.link.terminal(
                            &self.env,
                            address,
                            row["agent"].as_str().unwrap_or(""),
                            info,
                            &hooks,
                            &table,
                        )
                    });
                    if let Some(terminal) = terminal {
                        read.insert(key.clone(), terminal);
                    }
                    links.insert(key, link);
                }
                unique(&mut links);
                for row in &mut rows {
                    let key = row["pane_id"].as_str().unwrap().to_owned();
                    match &links[&key].1 {
                        Link::Linked(id) => {
                            row["agent_session"] =
                                json!({"kind":"id","value":id,"source":"panorama:core"});
                            if let Some(terminal) = read.remove(&key) {
                                let harness = row["agent"].as_str().unwrap_or("").to_owned();
                                let pid = links[&key].0;
                                let default_config = harness != "claude"
                                    || pid.is_some_and(|pid| self.link.default_config(pid));
                                observation.linked.push(Linked {
                                    address: row["terminal"].as_str().unwrap().to_owned(),
                                    harness,
                                    session: id.clone(),
                                    terminal,
                                    default_config,
                                });
                            }
                        }
                        Link::Conflict => observation
                            .errors
                            .push(format!("{key}: session evidence conflicts")),
                        Link::Unavailable => observation
                            .errors
                            .push(format!("{key}: session ownership unavailable")),
                    }
                }
                observation.snapshot = Some(json!({"agents":rows,"tabs":[],"panes":[]}));
            }
        }
        observation
    }
}

/// Actual hc CLI envelope (not its RPC Terminal schema). Never turn `where` into app membership.
pub fn normalize(value: Value) -> Result<Vec<Value>, &'static str> {
    let terminals = value
        .get("terminals")
        .and_then(Value::as_array)
        .ok_or("invalid hc list envelope")?;
    let mut rows = Vec::new();
    let mut addresses = HashSet::new();
    for terminal in terminals {
        let address = terminal
            .get("address")
            .and_then(Value::as_str)
            .filter(|s| valid_address(s))
            .ok_or("invalid hc terminal address")?;
        if !addresses.insert(address) {
            return Err("duplicate hc terminal address");
        }
        let harness = match terminal.get("agent").and_then(Value::as_str) {
            Some("claude-code" | "claude") => "claude",
            Some("codex") => "codex",
            _ => continue,
        };
        if terminal.get("state").and_then(Value::as_str) != Some("live") {
            continue;
        }
        let status = match terminal.get("status") {
            Some(Value::String(s)) => s.as_str(),
            Some(Value::Null) | None => "unknown",
            _ => return Err("invalid hc status"),
        };
        let cwd = match terminal.get("cwd") {
            Some(Value::String(s)) => Some(s),
            Some(Value::Null) | None => None,
            _ => return Err("invalid hc cwd"),
        };
        rows.push(
            json!({"pane_id":pane_key(address),"terminal":address,"agent":harness,
            "agent_status":status,"cwd":cwd,"tab_id":null,"workspace_id":null,"focused":false}),
        );
    }
    Ok(rows)
}

#[cfg(test)]
mod tests;
