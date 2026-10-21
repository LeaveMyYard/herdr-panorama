//! The harnesses the new-agent carousel shows (the design's `HARNESS`, original character
//! art) and which of them panorama can start yet (the design's `SUPPORTED`: Claude Code).

use crate::view::colour::Rgb;

/// One harness: its id, the name shown, the command it runs, its colour and its 13 × 7 art.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harness {
    pub id: &'static str,
    pub name: &'static str,
    pub cmd: &'static str,
    pub col: Rgb,
    pub big: [&'static str; 7],
}

impl Harness {
    /// Whether panorama starts this harness yet; the others say "Coming soon".
    pub fn supported(&self) -> bool {
        self.id == "claude"
    }
}

/// Every harness, in the carousel's order.
pub const HARNESSES: [Harness; 5] = [
    Harness {
        id: "claude",
        name: "Claude Code",
        cmd: "claude",
        col: Rgb::hex(0xe39b72),
        big: [
            "      │      ",
            "  ╲   │   ╱  ",
            "    ╲ │ ╱    ",
            " ─────●───── ",
            "    ╱ │ ╲    ",
            "  ╱   │   ╲  ",
            "      │      ",
        ],
    },
    Harness {
        id: "codex",
        name: "Codex",
        cmd: "codex",
        col: Rgb::hex(0x9db6ea),
        big: [
            "             ",
            "    ╭───╮    ",
            " ╭──╯   ╰─╮  ",
            "╭╯ ›_      ╰╮",
            "╰───────────╯",
            "             ",
            "             ",
        ],
    },
    Harness {
        id: "opencode",
        name: "OpenCode",
        cmd: "opencode",
        col: Rgb::hex(0xc9ced6),
        big: [
            "  ╭─     ─╮  ",
            "  │ ── ─  │  ",
            "  │ ───── │  ",
            "──┤  ── ─ ├──",
            "  │ ────  │  ",
            "  │ ─ ─── │  ",
            "  ╰─     ─╯  ",
        ],
    },
    Harness {
        id: "gemini",
        name: "Gemini CLI",
        cmd: "gemini",
        col: Rgb::hex(0xb4a6f0),
        big: [
            "     ╻     ✦ ",
            "     ┃       ",
            "    ╱ ╲      ",
            " ━━━   ━━━   ",
            "    ╲ ╱      ",
            "     ┃       ",
            "     ╹       ",
        ],
    },
    Harness {
        id: "droid",
        name: "Droid",
        cmd: "droid",
        col: Rgb::hex(0x8fd0a4),
        big: [
            "   ╷     ╷   ",
            " ╭─┴─────┴─╮ ",
            " │  ◉   ◉  │ ",
            " │   ───   │ ",
            " ╰─┬─────┬─╯ ",
            "  ╶┴╴   ╶┴╴  ",
            "             ",
        ],
    },
];

/// The harness `d` steps from `i`, wrapping round (the design's `step`).
pub fn step(i: usize, d: i32) -> usize {
    let n = HARNESSES.len() as i32;
    ((i as i32 + d).rem_euclid(n)) as usize
}
