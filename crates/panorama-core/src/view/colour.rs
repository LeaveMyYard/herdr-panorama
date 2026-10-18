//! A 24-bit colour, and moving one toward another (Python `render.mix`, `theme._mix`).

use std::fmt;

/// A 24-bit colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// From `0xrrggbb`.
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// From `#rrggbb` (any case); `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        let digits = s.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok().map(Self::hex)
    }

    /// This colour moved `k` of the way to `to`, each channel rounded half up (JS
    /// `Math.round`, Python `floor(x + 0.5)`), in the same float operations as Python.
    pub fn mix(self, to: Rgb, k: f64) -> Rgb {
        let ch = |a: u8, b: u8| {
            let (a, b) = (i32::from(a), i32::from(b));
            (f64::from(a) + f64::from(b - a) * k + 0.5).floor() as u8
        };
        Rgb(ch(self.0, to.0), ch(self.1, to.1), ch(self.2, to.2))
    }
}

impl fmt::Display for Rgb {
    /// `#rrggbb`, lower case, as Python writes colours.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[cfg(test)]
mod tests;
