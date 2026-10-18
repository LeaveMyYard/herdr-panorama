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
