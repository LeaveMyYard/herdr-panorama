use super::*;
use crate::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG};

const OWN: OwnColours = OwnColours {
    fg: FG,
    dim: DIM,
    faint: FAINT,
    fallback: (DOCK_HOV, BG),
};

fn herdr() -> PanelSettings {
    PanelSettings {
        follow_herdr: true,
        ..PanelSettings::default()
    }
}

#[test]
fn the_table_is_pythons() {
    // design_10.json's themes.tokyo, and theme.py's catppuccin (the default theme)
    let tokyo = Tokens::builtin("tokyo-night").unwrap();
    assert_eq!(
        tokyo,
        Tokens::new([0x1a1b26, 0x232636, 0xc0caf5, 0xa9b1d6, 0x697196, 0x565f89])
    );
    assert_eq!(
        Tokens::builtin("catppuccin").unwrap().panel_bg,
        Some(Rgb::hex(0x181825))
    );
    assert_eq!(THEMES.len(), 17);
    assert_eq!(Tokens::builtin("terminal"), None);
}

#[test]
fn fixed_is_the_default_and_own_colours_replace_either() {
    // tests/test_focus_theme.py::test_fixed_is_the_default_and_panoramas_own_colours_replace_either
    let tokyo = Tokens::builtin("tokyo-night").unwrap();
    let s = PanelSettings::default();
    assert_eq!(panel_look(Some(&tokyo), true, OWN, &s).bg, FIXED.0);
    let look = panel_look(Some(&tokyo), false, OWN, &s);
    assert_eq!((look.bg, look.fgs.len()), (FIXED.1, 0));
    assert_eq!(look.hover, Rgb::hex(0x1a1b1e));
    assert_eq!(panel_look(None, true, OWN, &s).hover, Rgb::hex(0x1d1f24));
    let own = PanelSettings {
        panel_bg: Some(Rgb::hex(0x000000)),
        focus_bg: Some(Rgb::hex(0x111111)),
        ..s
    };
    assert_eq!(panel_look(None, false, OWN, &own).bg, Rgb::hex(0x000000));
    assert_eq!(panel_look(None, true, OWN, &own).bg, Rgb::hex(0x111111));
    let own_herdr = PanelSettings {
        follow_herdr: true,
        ..own
    };
    assert_eq!(
        panel_look(Some(&tokyo), false, OWN, &own_herdr).bg,
        Rgb::hex(0x000000)
    );
    assert_eq!(
        panel_look(Some(&tokyo), true, OWN, &own_herdr).bg,
        Rgb::hex(0x111111)
    );
}

#[test]
fn the_herdr_mode_tints_toward_active_row_bg_and_maps_the_texts() {
    let tokyo = Tokens::builtin("tokyo-night").unwrap();
    let off = panel_look(Some(&tokyo), false, OWN, &herdr());
    assert_eq!(off.bg, Rgb::hex(0x1a1b26));
    assert_eq!(
        off.fgs,
        [
            (FG, Rgb::hex(0xc0caf5)),
            (DIM, Rgb::hex(0x697196)),
            (FAINT, Rgb::hex(0x565f89))
        ]
    );
    let on = panel_look(Some(&tokyo), true, OWN, &herdr());
    assert_eq!(on.bg, Rgb::hex(0x1a1b26).mix(Rgb::hex(0x232636), 0.15));
    assert_eq!(on.fgs[1], (DIM, Rgb::hex(0xa9b1d6)));
    let full = PanelSettings {
        focus_tint: 1.0,
        ..herdr()
    };
    assert_eq!(
        panel_look(Some(&tokyo), true, OWN, &full).bg,
        Rgb::hex(0x232636)
    );
}

#[test]
fn reset_tokens_and_the_terminal_theme_keep_panoramas_colours() {
    let mut t = Tokens::builtin("tokyo-night").unwrap();
    t.panel_bg = None;
    t.subtext0 = None;
    let on = panel_look(Some(&t), true, OWN, &herdr());
    assert_eq!(on.bg, BG.mix(Rgb::hex(0x232636), 0.15));
    assert!(on.fgs.iter().all(|(from, _)| *from != DIM));
    let term = panel_look(None, false, OWN, &herdr());
    assert_eq!((term.bg, term.fgs.len()), (BG, 0));
    assert_eq!(
        panel_look(None, true, OWN, &herdr()).bg,
        BG.mix(DOCK_HOV, 0.15)
    );
}

#[test]
fn every_built_in_theme_is_herdrs_with_six_tokens() {
    // tests/test_focus_theme.py::test_every_built_in_theme_has_the_six_tokens: herdr 0.9.3's
    // list, less "terminal"; every token set
    let mut names: Vec<&str> = THEMES.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    let mut want = "catppuccin catppuccin-latte tokyo-night tokyo-night-day dracula nord gruvbox \
        gruvbox-light one-dark one-light solarized solarized-light kanagawa kanagawa-lotus \
        rose-pine rose-pine-dawn vesper"
        .split_whitespace()
        .collect::<Vec<_>>();
    want.sort_unstable();
    assert_eq!(names, want);
    for (n, t) in THEMES {
        let all = [
            t.panel_bg,
            t.active_row_bg,
            t.text,
            t.subtext0,
            t.overlay1,
            t.overlay0,
        ];
        assert!(all.iter().all(Option::is_some), "{n}");
    }
}

#[test]
fn the_table_is_herdrs_measured_values() {
    // tests/test_focus_theme.py::test_the_table_is_herdrs_measured_values (review of #19):
    // herdr 0.9.3's own values, measured per token
    let measured: &[(&str, &[(&str, u32)])] = &[
        (
            "catppuccin",
            &[("panel_bg", 0x181825), ("active_row_bg", 0x1e1e2e)],
        ),
        ("catppuccin-latte", &[("active_row_bg", 0xe6e9ef)]),
        ("tokyo-night", &[("active_row_bg", 0x232636)]),
        (
            "tokyo-night-day",
            &[("active_row_bg", 0xd2d3da), ("overlay0", 0x8990b3)],
        ),
        (
            "dracula",
            &[
                ("active_row_bg", 0x373c52),
                ("subtext0", 0xd2d2dc),
                ("overlay1", 0x828cb4),
                ("overlay0", 0x6272a4),
            ],
        ),
        (
            "nord",
            &[("active_row_bg", 0x434c5e), ("overlay1", 0x646e82)],
        ),
        (
            "gruvbox",
            &[
                ("active_row_bg", 0x323130),
                ("overlay1", 0xa89984),
                ("overlay0", 0x928374),
            ],
        ),
        (
            "gruvbox-light",
            &[
                ("active_row_bg", 0xf2e5bc),
                ("overlay1", 0x7c6f64),
                ("overlay0", 0x928374),
            ],
        ),
        (
            "one-dark",
            &[
                ("active_row_bg", 0x313640),
                ("subtext0", 0x969ca8),
                ("overlay1", 0x737a87),
            ],
        ),
        (
            "one-light",
            &[("active_row_bg", 0xd8dbe2), ("subtext0", 0x686b77)],
        ),
        ("solarized", &[("active_row_bg", 0x164b57)]),
        (
            "solarized-light",
            &[
                ("text", 0x657b83),
                ("subtext0", 0x839496),
                ("overlay1", 0x586e75),
            ],
        ),
        (
            "kanagawa",
            &[
                ("subtext0", 0xc8c3aa),
                ("overlay1", 0x87867d),
                ("overlay0", 0x727169),
            ],
        ),
        (
            "kanagawa-lotus",
            &[("active_row_bg", 0xd5cea3), ("subtext0", 0x43436c)],
        ),
        (
            "rose-pine",
            &[
                ("active_row_bg", 0x26233a),
                ("subtext0", 0xc8c5dc),
                ("overlay1", 0x908caa),
                ("overlay0", 0x6e6a86),
            ],
        ),
        (
            "rose-pine-dawn",
            &[
                ("active_row_bg", 0xe3d9cf),
                ("text", 0x464261),
                ("overlay1", 0x797593),
                ("overlay0", 0x9893a5),
            ],
        ),
        (
            "vesper",
            &[
                ("panel_bg", 0x1a1a1a),
                ("active_row_bg", 0x101010),
                ("overlay1", 0x7e7e7e),
                ("overlay0", 0x5c5c5c),
            ],
        ),
    ];
    for (name, tokens) in measured {
        let t = Tokens::builtin(name).unwrap();
        for (token, hex) in *tokens {
            let got = match *token {
                "panel_bg" => t.panel_bg,
                "active_row_bg" => t.active_row_bg,
                "text" => t.text,
                "subtext0" => t.subtext0,
                "overlay1" => t.overlay1,
                _ => t.overlay0,
            };
            assert_eq!(got, Some(Rgb::hex(*hex)), "{name} {token}");
        }
    }
}
