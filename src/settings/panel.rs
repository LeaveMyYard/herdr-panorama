//! panorama's own panel settings in its `config.toml` (Python `theme.mode`, `focus_tint`,
//! `own_colors`, `config.animation_fps`). Each key is read on its own; a value of the wrong
//! kind takes the default, as in Python.

use super::colour::{ColourValue, colour};
use panorama_core::view::theme::{DEFAULT_TINT, PanelSettings};
use serde_json::{Map, Value};

/// `animation_fps`'s default and range.
pub const DEFAULT_FPS: f64 = 20.0;
pub const FPS_RANGE: (f64, f64) = (2.0, 30.0);

/// A number as Python's `float()` takes it: a number, a bool (1 or 0), or a numeric string.
fn float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn own_colour(cfg: &Map<String, Value>, key: &str) -> Option<panorama_core::view::Rgb> {
    match cfg.get(key).map(colour) {
        Some(ColourValue::Colour(c)) => Some(c),
        _ => None,
    }
}

/// The panel's colour settings: `panel_colors` ("herdr" follows herdr's theme, anything else
/// is fixed), `focus_tint` (0..1), `panel_bg` / `focus_bg`.
pub fn panel_settings(cfg: &Map<String, Value>) -> PanelSettings {
    let follow_herdr = match cfg.get("panel_colors") {
        Some(Value::String(s)) => s.to_lowercase() == "herdr",
        _ => false,
    };
    let tint = cfg
        .get("focus_tint")
        .map_or(Some(DEFAULT_TINT), float)
        .filter(|t| !t.is_nan());
    PanelSettings {
        follow_herdr,
        focus_tint: tint.unwrap_or(DEFAULT_TINT).clamp(0.0, 1.0),
        panel_bg: own_colour(cfg, "panel_bg"),
        focus_bg: own_colour(cfg, "focus_bg"),
    }
}

/// `animation_fps`, 2..30 (a bool or a non-number: the default).
pub fn animation_fps(cfg: &Map<String, Value>) -> f64 {
    match cfg.get("animation_fps") {
        None => DEFAULT_FPS,
        Some(Value::Number(n)) => n
            .as_f64()
            .map_or(DEFAULT_FPS, |v| v.clamp(FPS_RANGE.0, FPS_RANGE.1)),
        Some(_) => DEFAULT_FPS,
    }
}

#[cfg(test)]
mod tests;
