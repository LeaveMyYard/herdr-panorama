//! panorama's own colours (Python `render.py`: the design's `TH.dark`, the status colours
//! `ST` and the tile colours `PB`).

use super::colour::Rgb;
use crate::model::Status;

/// The background.
pub const BG: Rgb = Rgb::hex(0x101215);
/// Text.
pub const FG: Rgb = Rgb::hex(0xc3c7cd);
/// Highlighted text.
pub const HI: Rgb = Rgb::hex(0xeceef1);
/// Secondary text.
pub const DIM: Rgb = Rgb::hex(0x80868f);
/// Tertiary text.
pub const FAINT: Rgb = Rgb::hex(0x4d535c);
/// Lines and borders.
pub const LINE: Rgb = Rgb::hex(0x2c313a);
/// The docked entry's background.
pub const SEL: Rgb = Rgb::hex(0x1e2937);
/// The graph header's background.
pub const HDR: Rgb = Rgb::hex(0x171a1f);
/// Keys and the docked bar.
pub const KEY: Rgb = Rgb::hex(0xe2cf9c);
/// The docked list's hover row (swarm-dock.js `HOV`).
pub const DOCK_HOV: Rgb = Rgb::hex(0x171a1f);
/// A needs-you tile in its off phase: (background, glyph) (swarm-tiles.js `OFF`).
pub const TILE_OFF: (Rgb, Rgb) = (Rgb::hex(0x2a2e35), Rgb::hex(0x4d535c));
/// Dark text on a light tile (the "refresh failed" badge).
pub const ON_LIGHT: Rgb = Rgb::hex(0x1b1203);

/// A status's colour (`ST`); text colour for an unknown code (`ST.get(st, FG)`).
pub fn status(st: &Status) -> Rgb {
    match st {
        Status::Queued => Rgb::hex(0x8d939d),
        Status::Running => Rgb::hex(0x68c2b5),
        Status::Reviewing => Rgb::hex(0x8eaee6),
        Status::Waiting | Status::Blocked => Rgb::hex(0xffb547),
        Status::Done => Rgb::hex(0x6c9673),
        Status::Failed => Rgb::hex(0xff6b6b),
        Status::Cancelled | Status::Stale => Rgb::hex(0x646a73),
        Status::Inferred => Rgb::hex(0x8a9a8d),
        Status::Lost => Rgb::hex(0xc08a8a),
        Status::Background => Rgb::hex(0xb39ddb),
        Status::Unknown(_) => FG,
    }
}

/// A status's tile: (background, glyph) (`PB`; queued's for an unknown code).
pub fn tile(st: &Status) -> (Rgb, Rgb) {
    let (b, f) = match st {
        Status::Done => (0x2c6b4c, 0xe3f2e9),
        Status::Running => (0x1f625c, 0xd8f2ee),
        Status::Reviewing => (0x2e4f86, 0xdfe8f8),
        Status::Waiting | Status::Blocked => (0xf2a93b, 0x1b1203),
        Status::Failed => (0xd8363e, 0xffffff),
        Status::Cancelled => (0x24272c, 0x6b717a),
        Status::Inferred => (0x26312a, 0xa9bcae),
        Status::Lost => (0x3a2526, 0xe0b4b4),
        Status::Background => (0x3b3159, 0xe6def8),
        Status::Stale => (0x1b1e23, 0x6b717a),
        Status::Queued | Status::Unknown(_) => (0x2a2e35, 0x9aa0aa),
    };
    (Rgb::hex(b), Rgb::hex(f))
}
