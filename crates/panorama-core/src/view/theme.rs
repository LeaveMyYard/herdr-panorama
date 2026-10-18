//! The docked panel's look from herdr's theme, focused or not (design turn 10,
//! `docs/design/swarm-focus.js`; Python `theme.py`'s table and `panel_look`).
//!
//! Reading herdr's `config.toml` and panorama's own settings is the binary's job; this module
//! holds the measured theme table and turns tokens plus settings into colours.

mod table;

pub use table::{THEMES, Tokens};

use super::colour::Rgb;

/// The operator's fixed backgrounds, (focused, not focused).
pub const FIXED: (Rgb, Rgb) = (Rgb::hex(0x14161b), Rgb::hex(0x101215));
/// `focus_tint`'s default.
pub const DEFAULT_TINT: f64 = 0.15;
/// The hover row: this far from the panel's background toward white.
pub const HOVER_STEP: f64 = 0.04;

/// panorama's own colour settings (its `config.toml`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelSettings {
    /// `panel_colors = "herdr"`: follow herdr's theme; else the fixed colours.
    pub follow_herdr: bool,
    /// `focus_tint`, 0..1.
    pub focus_tint: f64,
    /// `panel_bg`: the unfocused background, replacing either mode's.
    pub panel_bg: Option<Rgb>,
    /// `focus_bg`: the focused background, replacing either mode's.
    pub focus_bg: Option<Rgb>,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            follow_herdr: false,
            focus_tint: DEFAULT_TINT,
            panel_bg: None,
            focus_bg: None,
        }
    }
}

/// What the renderer needs: the panel's background, panorama's text colours mapped to the
/// theme's (`(from, to)` pairs), and the hover row's background.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelLook {
    pub bg: Rgb,
    pub fgs: Vec<(Rgb, Rgb)>,
    pub hover: Rgb,
}

/// The texts panorama draws that a theme recolours, and the fallback backgrounds (active,
/// not focused) for a theme without them.
#[derive(Clone, Copy, Debug)]
pub struct OwnColours {
    pub fg: Rgb,
    pub dim: Rgb,
    pub faint: Rgb,
    pub fallback: (Rgb, Rgb),
}

/// The hover row: a step lighter than the panel's background, so it shows on both.
pub fn hover_bg(bg: Rgb) -> Rgb {
    bg.mix(Rgb::hex(0xffffff), HOVER_STEP)
}

/// The panel's colours (Python `panel_colors` plus `hover_bg`). `tokens` `None`: herdr's
/// "terminal" theme (no palette to read).
///
/// Fixed (the default): [`FIXED`] and panorama's own text colours. Following herdr: not
/// focused, the theme's `panel_bg`, text, dim -> `overlay1`, faint -> `overlay0`; focused,
/// that background moved `focus_tint` toward `active_row_bg`, dim -> `subtext0`, faint ->
/// `overlay1`. In both, panorama's own `panel_bg` / `focus_bg` replace the backgrounds.
pub fn panel_look(
    tokens: Option<&Tokens>,
    focused: bool,
    own: OwnColours,
    s: &PanelSettings,
) -> PanelLook {
    let bg = if !s.follow_herdr {
        if focused {
            s.focus_bg.unwrap_or(FIXED.0)
        } else {
            s.panel_bg.unwrap_or(FIXED.1)
        }
    } else {
        let base = s
            .panel_bg
            .or(tokens.and_then(|t| t.panel_bg))
            .unwrap_or(own.fallback.1);
        match (focused, s.focus_bg) {
            (false, _) => base,
            (true, Some(f)) => f,
            (true, None) => {
                let active = tokens
                    .and_then(|t| t.active_row_bg)
                    .unwrap_or(own.fallback.0);
                base.mix(active, s.focus_tint.clamp(0.0, 1.0))
            }
        }
    };
    let fgs = match tokens {
        Some(t) if s.follow_herdr => {
            let (dim, faint) = if focused {
                (t.subtext0, t.overlay1)
            } else {
                (t.overlay1, t.overlay0)
            };
            [(own.fg, t.text), (own.dim, dim), (own.faint, faint)]
                .into_iter()
                .filter_map(|(from, to)| to.map(|to| (from, to)))
                .collect()
        }
        _ => vec![],
    };
    PanelLook {
        bg,
        fgs,
        hover: hover_bg(bg),
    }
}

#[cfg(test)]
mod tests;
