//! The key bound to panorama's open action, as herdr's `config.toml` has it (issue #13;
//! Python `openkey.py`), for the docked panel's footer: `[[keys.command]]` with
//! `type = "plugin_action"` and `command = "<plugin id>.open"`.
//!
//! `(full, short)`: a prefix binding reads `pfx+` then the rest, modifiers are letters joined
//! with `-` (`ctrl+alt+p` -> `c-a-p`). The suggested `prefix+space`, and no binding at all,
//! show the design's own `pfx+spc`.

use herdr_agent_kit::herdr_api::clean;
use serde_json::{Map, Value};

/// The design's footer text, for the suggested binding and for none.
pub const DEFAULT: (&str, &str) = ("pfx+spc", "pfx+spc");
const SUGGESTED: &str = "prefix+space";

fn word(p: &str) -> &str {
    match p {
        "space" => "spc",
        "ctrl" => "c",
        "alt" => "a",
        "shift" => "s",
        "super" | "cmd" => "d",
        "enter" => "ret",
        "escape" => "esc",
        "backspace" => "bs",
        other => other,
    }
}

/// A key's short form.
pub fn short(key: &str) -> String {
    let lower = key.trim().to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').filter(|p| !p.is_empty()).collect();
    let head = if parts.first() == Some(&"prefix") {
        parts.remove(0);
        "pfx+"
    } else {
        ""
    };
    format!(
        "{head}{}",
        parts.into_iter().map(word).collect::<Vec<_>>().join("-")
    )
}

/// The key herdr binds to `<plugin_id>.open`, cleaned (at most 40 characters).
pub fn binding(cfg: &Map<String, Value>, plugin_id: &str) -> Option<String> {
    let action = format!("{plugin_id}.open");
    let entries = cfg.get("keys")?.get("command")?.as_array()?;
    entries.iter().find_map(|e| {
        let ours =
            e.get("type")?.as_str()? == "plugin_action" && e.get("command")?.as_str()? == action;
        let key = e.get("key")?.as_str()?.trim();
        (ours && !key.is_empty()).then(|| clean(key, 40))
    })
}

/// The footer's `(full, short)` for herdr's config `cfg`.
pub fn current(cfg: &Map<String, Value>, plugin_id: &str) -> (String, String) {
    match binding(cfg, plugin_id) {
        Some(key) if key.to_lowercase() != SUGGESTED => {
            let s = short(&key);
            (key, s)
        }
        _ => (DEFAULT.0.to_owned(), DEFAULT.1.to_owned()),
    }
}

#[cfg(test)]
mod tests;
