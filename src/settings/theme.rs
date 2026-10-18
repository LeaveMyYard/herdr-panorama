//! herdr's theme, read from herdr's own `config.toml` (Python `theme.resolve`): `[theme]
//! name` (default catppuccin; with `auto_switch`, its `dark_name`, since panorama's own
//! colours are dark), then the `[theme.custom]` overrides (and `[theme.custom.dark]` with
//! `auto_switch`).

use super::colour::{ColourValue, colour};
use panorama_core::view::Tokens;
use serde_json::{Map, Value};

/// herdr's default theme.
pub const DEFAULT: &str = "catppuccin";

fn set(tokens: &mut Tokens, layer: &Map<String, Value>) {
    let slots: [(&str, &mut Option<_>); 6] = [
        ("panel_bg", &mut tokens.panel_bg),
        ("active_row_bg", &mut tokens.active_row_bg),
        ("text", &mut tokens.text),
        ("subtext0", &mut tokens.subtext0),
        ("overlay1", &mut tokens.overlay1),
        ("overlay0", &mut tokens.overlay0),
    ];
    for (key, slot) in slots {
        match layer.get(key).map(colour) {
            Some(ColourValue::Colour(c)) => *slot = Some(c),
            Some(ColourValue::Reset) => *slot = None,
            Some(ColourValue::Invalid) | None => {}
        }
    }
}

/// The six tokens for herdr's config `cfg`; `None` for the "terminal" theme (no palette to
/// read). An unknown theme name reads as the default theme.
pub fn tokens(cfg: &Map<String, Value>) -> Option<Tokens> {
    let empty = Map::new();
    let th = cfg
        .get("theme")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let auto = th.get("auto_switch") == Some(&Value::Bool(true));
    // Python's `or`: a missing, null, false or empty name falls through. Python treats a 0,
    // [] or {} name as falsy too; here those read as a name, which is unknown, so the default
    // theme: only an absurd config tells the two apart.
    let truthy =
        |v: &&Value| !matches!(v, Value::Null | Value::Bool(false)) && v.as_str() != Some("");
    let pick = if auto {
        th.get("dark_name").filter(truthy)
    } else {
        None
    };
    let name = match pick.or_else(|| th.get("name").filter(truthy)) {
        Some(Value::String(s)) => s.to_lowercase(),
        Some(other) => other.to_string().to_lowercase(),
        None => DEFAULT.to_owned(),
    };
    if name == "terminal" {
        return None;
    }
    let mut t = Tokens::builtin(&name)
        .or_else(|| Tokens::builtin(DEFAULT))
        .expect("the default theme is built in");
    if let Some(custom) = th.get("custom").and_then(Value::as_object) {
        set(&mut t, custom);
        if auto && let Some(dark) = custom.get("dark").and_then(Value::as_object) {
            set(&mut t, dark);
        }
    }
    Some(t)
}

#[cfg(test)]
mod tests;
