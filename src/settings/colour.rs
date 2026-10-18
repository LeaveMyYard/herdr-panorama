//! A colour value as herdr's config writes it (Python `theme.color`): `#rrggbb`,
//! `rgb(r, g, b)`, a few names, or a reset word.

use panorama_core::view::Rgb;
use serde_json::Value;

/// What a colour setting says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourValue {
    /// A colour.
    Colour(Rgb),
    /// `reset`, `default`, `none` or `transparent`: unset.
    Reset,
    /// Not a colour (the setting is ignored).
    Invalid,
}

const NAMED: [(&str, u32); 10] = [
    ("black", 0x000000),
    ("white", 0xffffff),
    ("red", 0xff0000),
    ("green", 0x00ff00),
    ("blue", 0x0000ff),
    ("yellow", 0xffff00),
    ("cyan", 0x00ffff),
    ("magenta", 0xff00ff),
    ("gray", 0x808080),
    ("grey", 0x808080),
];

fn rgb_call(v: &str) -> Option<Rgb> {
    let inner = v.strip_prefix("rgb(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let [r, g, b] = parts.as_slice() else {
        return None;
    };
    let ch = |s: &str| -> Option<u8> {
        (1..=3).contains(&s.len()).then_some(())?;
        s.bytes().all(|b| b.is_ascii_digit()).then_some(())?;
        s.parse::<u16>().ok().and_then(|n| u8::try_from(n).ok())
    };
    Some(Rgb(ch(r)?, ch(g)?, ch(b)?))
}

/// The value of a colour setting.
pub fn colour(value: &Value) -> ColourValue {
    let Some(s) = value.as_str() else {
        return ColourValue::Invalid;
    };
    let v = s.trim().to_lowercase();
    if ["reset", "default", "none", "transparent"].contains(&v.as_str()) {
        return ColourValue::Reset;
    }
    if let Some(c) = Rgb::parse(&v).or_else(|| rgb_call(&v)) {
        return ColourValue::Colour(c);
    }
    NAMED
        .iter()
        .find(|(n, _)| *n == v)
        .map_or(ColourValue::Invalid, |(_, c)| {
            ColourValue::Colour(Rgb::hex(*c))
        })
}

#[cfg(test)]
mod tests;
