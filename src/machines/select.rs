//! Which machines panorama shows (Python `machines.selection`, `saved`): the `machines`
//! setting in panorama's `config.toml` over herdr's saved machines (`herdr machine list
//! --json`).

use super::{Machine, safe_target};
use herdr_agent_kit::herdr_api::Machine as Saved;
use serde_json::{Map, Value};

/// The `machines` setting: `"all"` (the default), `"none"`, or a list of labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Every enabled saved machine.
    All,
    /// No machine.
    None,
    /// The enabled saved machines with these labels.
    Labels(Vec<String>),
}

/// Python's `str()` of a TOML scalar, for a label in the list.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        other => other.to_string(),
    }
}

impl Selection {
    /// The setting as Python reads it: a list is labels; `none`, `off`, `false`, `0` or empty
    /// (trimmed, any case) is none; anything else, or no setting, is all.
    pub fn from_config(cfg: &Map<String, Value>) -> Self {
        match cfg.get("machines") {
            None => Self::All,
            Some(Value::Array(labels)) => Self::Labels(labels.iter().map(py_str).collect()),
            Some(v) => match py_str(v).trim().to_lowercase().as_str() {
                "none" | "off" | "false" | "0" | "" => Self::None,
                _ => Self::All,
            },
        }
    }
}

/// Where the setting and herdr's saved machines come from: the config file and the herdr CLI,
/// or a test's script.
pub trait Source: Send {
    /// The `machines` setting now (the config file is re-read when it changed).
    fn selection(&mut self) -> Selection;
    /// herdr's saved machines, unfiltered; none when herdr cannot say.
    fn machines(&mut self) -> Vec<Saved>;
}

/// The saved machines to watch: enabled, with an id and a safe ssh target (one that would read
/// as an option is never used), filtered by the setting. A label defaults to the id.
pub fn saved(source: &mut dyn Source) -> Vec<Machine> {
    let sel = source.selection();
    if sel == Selection::None {
        return Vec::new();
    }
    let set = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    source
        .machines()
        .into_iter()
        .filter(|r| r.enabled)
        .filter_map(|r| {
            let (id, target) = (set(&r.id)?, set(&r.target)?);
            let label = set(&r.label).unwrap_or_else(|| id.clone());
            Some(Machine { id, label, target })
        })
        .filter(|m| safe_target(&m.target))
        .filter(|m| match &sel {
            Selection::Labels(labels) => labels.contains(&m.label),
            _ => true,
        })
        .collect()
}

#[cfg(test)]
mod tests;
