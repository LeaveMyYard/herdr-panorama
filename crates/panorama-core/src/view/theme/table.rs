//! herdr's built-in themes: the six tokens the panel reads (Python `theme.THEMES`).
//!
//! herdr 0.9.3's own values, measured (review of #19): an isolated herdr per theme with a
//! client attached, rendered plain and once per token with that token overridden by a
//! sentinel colour; the colour that disappears is herdr's value. "terminal" follows the host
//! terminal's palette, which panorama cannot read: it has no entry.

use crate::view::colour::Rgb;

/// A theme's tokens; `None`: unset or reset (the panel then keeps panorama's own colour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub panel_bg: Option<Rgb>,
    pub active_row_bg: Option<Rgb>,
    pub text: Option<Rgb>,
    pub subtext0: Option<Rgb>,
    pub overlay1: Option<Rgb>,
    pub overlay0: Option<Rgb>,
}

impl Tokens {
    /// The six tokens, in the order above, all set.
    pub const fn new(v: [u32; 6]) -> Self {
        Self {
            panel_bg: Some(Rgb::hex(v[0])),
            active_row_bg: Some(Rgb::hex(v[1])),
            text: Some(Rgb::hex(v[2])),
            subtext0: Some(Rgb::hex(v[3])),
            overlay1: Some(Rgb::hex(v[4])),
            overlay0: Some(Rgb::hex(v[5])),
        }
    }

    /// A built-in theme by name (exact, lower case).
    pub fn builtin(name: &str) -> Option<Tokens> {
        THEMES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
    }
}

/// The built-in themes, by herdr's names.
pub const THEMES: [(&str, Tokens); 17] = [
    (
        "catppuccin",
        Tokens::new([0x181825, 0x1e1e2e, 0xcdd6f4, 0xa6adc8, 0x7f849c, 0x6c7086]),
    ),
    (
        "catppuccin-latte",
        Tokens::new([0xeff1f5, 0xe6e9ef, 0x4c4f69, 0x6c6f85, 0x8c8fa1, 0x9ca0b0]),
    ),
    (
        "tokyo-night",
        Tokens::new([0x1a1b26, 0x232636, 0xc0caf5, 0xa9b1d6, 0x697196, 0x565f89]),
    ),
    (
        "tokyo-night-day",
        Tokens::new([0xe1e2e7, 0xd2d3da, 0x3760bf, 0x6172b0, 0x68709a, 0x8990b3]),
    ),
    (
        "dracula",
        Tokens::new([0x282a36, 0x373c52, 0xf8f8f2, 0xd2d2dc, 0x828cb4, 0x6272a4]),
    ),
    (
        "nord",
        Tokens::new([0x2e3440, 0x434c5e, 0xeceff4, 0xd8dee9, 0x646e82, 0x4c566a]),
    ),
    (
        "gruvbox",
        Tokens::new([0x282828, 0x323130, 0xebdbb2, 0xd5c4a1, 0xa89984, 0x928374]),
    ),
    (
        "gruvbox-light",
        Tokens::new([0xfbf1c7, 0xf2e5bc, 0x3c3836, 0x504945, 0x7c6f64, 0x928374]),
    ),
    (
        "one-dark",
        Tokens::new([0x282c34, 0x313640, 0xabb2bf, 0x969ca8, 0x737a87, 0x5c6370]),
    ),
    (
        "one-light",
        Tokens::new([0xfafafa, 0xd8dbe2, 0x383a42, 0x686b77, 0x686b77, 0xa0a1a7]),
    ),
    (
        "solarized",
        Tokens::new([0x002b36, 0x164b57, 0x93a1a1, 0x839496, 0x657b83, 0x586e75]),
    ),
    (
        "solarized-light",
        Tokens::new([0xfdf6e3, 0xeee8d5, 0x657b83, 0x839496, 0x586e75, 0x93a1a1]),
    ),
    (
        "kanagawa",
        Tokens::new([0x1f1f28, 0x363646, 0xdcd7ba, 0xc8c3aa, 0x87867d, 0x727169]),
    ),
    (
        "kanagawa-lotus",
        Tokens::new([0xf2ecbc, 0xd5cea3, 0x545464, 0x43436c, 0x8a8980, 0xa09cac]),
    ),
    (
        "rose-pine",
        Tokens::new([0x191724, 0x26233a, 0xe0def4, 0xc8c5dc, 0x908caa, 0x6e6a86]),
    ),
    (
        "rose-pine-dawn",
        Tokens::new([0xfaf4ed, 0xe3d9cf, 0x464261, 0x797593, 0x797593, 0x9893a5]),
    ),
    (
        "vesper",
        Tokens::new([0x1a1a1a, 0x101010, 0xffffff, 0xa0a0a0, 0x7e7e7e, 0x5c5c5c]),
    ),
];
