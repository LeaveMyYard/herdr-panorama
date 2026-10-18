//! The home tab's and the cockpit's settings in panorama's `config.toml` (Python
//! `home.enabled`, `cockpit.enabled`, `per_tab`, `keep_focus_setting`, `arrows_setting`,
//! `follow_tabs`), read as Python reads them: through `str(value).lower()`.

use serde_json::{Map, Value};

/// Python's `str(value).lower()` for a TOML value (`True` → "true", `1.0` → "1.0").
fn py_str(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.to_lowercase(),
        Value::Number(n) => n.to_string(),
        Value::Null => "none".into(),
        other => other.to_string(),
    }
}

/// On unless it says off (`false`, `0`, `no`, `off`); missing is on.
fn on_unless_off(cfg: &Map<String, Value>, key: &str) -> bool {
    cfg.get(key).is_none_or(|v| !matches!(py_str(v).as_str(), "false" | "0" | "no" | "off"))
}

/// `home_tab` (default true): the open key shows a home tab; off, the popup.
pub fn home_enabled(cfg: &Map<String, Value>) -> bool {
    on_unless_off(cfg, "home_tab")
}

/// `cockpit` (default false, experimental), and only with the home tab: per-tab panels, the
/// only kind of cockpit (Q187; `cockpit_panels` is not read).
pub fn per_tab(cfg: &Map<String, Value>) -> bool {
    let on = cfg.get("cockpit").is_some_and(|v| matches!(py_str(v).as_str(), "true" | "1" | "yes" | "on"));
    home_enabled(cfg) && on
}

/// `dock_keep_focus` (default true): switching chats leaves the keyboard on the panels.
pub fn keep_focus(cfg: &Map<String, Value>) -> bool {
    on_unless_off(cfg, "dock_keep_focus")
}

/// `dock_on_arrows` (default true): ↑/↓ in a panel switch the chats. Needs `dock_keep_focus`.
pub fn arrows(cfg: &Map<String, Value>) -> bool {
    on_unless_off(cfg, "dock_on_arrows") && keep_focus(cfg)
}

/// `cockpit_follow_tabs = "single"`: panels only in tabs where the orchestrator is the only
/// pane ("any", the default, otherwise).
pub fn follow_tabs_single(cfg: &Map<String, Value>) -> bool {
    cfg.get("cockpit_follow_tabs").is_some_and(|v| py_str(v) == "single")
}
