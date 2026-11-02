//! Local terminal capability hints, sampled once when Settings is constructed. Presence
//! of a font alone cannot establish terminal mapping; unfamiliar setups use text.

use herdr_agent_kit::plugin_env::PluginEnv;
use panorama_core::view::harness::HarnessIcons;
use serde_json::{Map, Value};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

const FAMILY: &str = "Herdr Agent Icons Max";
const MAX_CONFIG: u64 = 64 * 1024;

/// Explicit font mode is an operator assertion about the terminal, so it bypasses auto.
/// Invalid values behave like auto. The config is re-read normally; auto is cached.
pub fn resolve(cfg: &Map<String, Value>, auto: bool) -> HarnessIcons {
    match cfg.get("harness_icons").and_then(Value::as_str) {
        Some("font") => HarnessIcons::Font,
        Some("text") => HarnessIcons::Text,
        _ if auto => HarnessIcons::Font,
        _ => HarnessIcons::Text,
    }
}

/// Conservative support for the standard local Ghostty setup installed by herdr-radar.
/// No subprocesses, recursive scans, config includes or fontconfig queries. Other terminals
/// and custom font locations can opt in explicitly. Restart to refresh the auto hint.
pub(super) fn detect(env: &PluginEnv) -> bool {
    if !matches!(env.var("TERM_PROGRAM"), Some("ghostty" | "herdr"))
        || env.var_os("SSH_CONNECTION").is_some()
    {
        return false;
    }
    let Some(home) = env.home() else {
        return false;
    };
    let ghostty = env.xdg_config_home().join("ghostty");
    let mapping = [ghostty.join("config"), ghostty.join("config.ghostty")]
        .iter()
        .any(|path| mapped(path));
    mapping
        && [
            home.join(".local/share/fonts"),
            home.join(".fonts"),
            home.join("Library/Fonts"),
        ]
        .iter()
        .any(|dir| installed(dir))
}

fn mapped(path: &Path) -> bool {
    if !fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_CONFIG) {
        return false;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut text = String::new();
    if file.take(MAX_CONFIG + 1).read_to_string(&mut text).is_err()
        || text.len() as u64 > MAX_CONFIG
    {
        return false;
    }
    text.lines().any(|line| {
        let line = line.split('#').next().unwrap_or("");
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        let Some((range, family)) = value.split_once('=') else {
            return false;
        };
        key.trim() == "font-codepoint-map"
            && range.trim().eq_ignore_ascii_case("U+E1A0-U+E1BA")
            && family.trim() == FAMILY
    })
}

fn installed(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.take(256).flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        name.starts_with("HerdrAgentIconsMax")
            && name.ends_with(".ttf")
            && entry.metadata().is_ok_and(|m| m.is_file() && m.len() > 0)
    })
}

#[cfg(test)]
mod tests;
