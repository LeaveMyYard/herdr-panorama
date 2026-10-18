//! tests/test_focus_theme.py's config cases, ported.

use super::*;
use panorama_core::view::Rgb;
use serde_json::json;

fn read(v: Value) -> Option<Tokens> {
    tokens(v.as_object().unwrap())
}

#[test]
fn no_config_is_herdrs_default_theme() {
    assert_eq!(read(json!({})), Tokens::builtin("catppuccin"));
    assert_eq!(
        read(json!({"theme": {"name": "Tokyo-Night"}})),
        Tokens::builtin("tokyo-night")
    );
    assert_eq!(
        read(json!({"theme": {"name": "no-such"}})),
        Tokens::builtin("catppuccin")
    );
    assert_eq!(read(json!({"theme": {"name": "terminal"}})), None);
}

#[test]
fn herdrs_custom_layer_overrides_the_built_in_theme() {
    let t = read(json!({"theme": {"name": "nord", "custom": {"panel_bg": "#010203", "text": "reset", "overlay0": "nope"}}}))
        .unwrap();
    let nord = Tokens::builtin("nord").unwrap();
    assert_eq!(t.panel_bg, Some(Rgb::hex(0x010203)));
    assert_eq!(t.text, None);
    assert_eq!(t.overlay0, nord.overlay0);
}

#[test]
fn layers_name_custom_then_dark_with_auto_switch() {
    let cfg = json!({"theme": {"name": "catppuccin-latte", "dark_name": "dracula", "auto_switch": true,
        "custom": {"panel_bg": "#000001", "dark": {"panel_bg": "#000002"}}}});
    let t = read(cfg).unwrap();
    assert_eq!(t.panel_bg, Some(Rgb::hex(0x000002)));
    assert_eq!(t.text, Tokens::builtin("dracula").unwrap().text);
    // without auto_switch: the name, and no dark layer
    let t = read(
        json!({"theme": {"name": "catppuccin-latte", "dark_name": "dracula",
        "custom": {"dark": {"panel_bg": "#000002"}}}}),
    )
    .unwrap();
    assert_eq!(t, Tokens::builtin("catppuccin-latte").unwrap());
}

#[test]
fn pythons_layer_and_reset_cases() {
    // test_layers_name_custom_then_dark_with_auto_switch
    let t = read(
        json!({"theme": {"name": "gruvbox", "auto_switch": true, "dark_name": "nord",
        "custom": {"text": "rgb(1,2,3)", "panel_bg": "Red", "dark": {"text": "#ABCDEF"}}}}),
    )
    .unwrap();
    assert_eq!(
        t.active_row_bg,
        Tokens::builtin("nord").unwrap().active_row_bg
    );
    assert_eq!(
        (t.panel_bg, t.text),
        (Some(Rgb::hex(0xff0000)), Some(Rgb::hex(0xabcdef)))
    );
    // test_reset_and_unknown_values
    let t = read(json!({"theme": {"name": "nope", "custom": {"panel_bg": "reset", "text": "chartreuse-ish"}}})).unwrap();
    assert_eq!(t.panel_bg, None);
    assert_eq!(t.text, Tokens::builtin("catppuccin").unwrap().text);
}
