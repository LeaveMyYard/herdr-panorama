use super::*;
use panorama_core::view::Rgb;
use serde_json::json;

fn cfg(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn fixed_is_the_default() {
    assert_eq!(panel_settings(&cfg(json!({}))), PanelSettings::default());
    assert!(!panel_settings(&cfg(json!({"panel_colors": "fixed"}))).follow_herdr);
    assert!(panel_settings(&cfg(json!({"panel_colors": "HERDR"}))).follow_herdr);
}

#[test]
fn focus_tint_takes_what_python_float_takes_and_clamps() {
    let tint = |v: Value| panel_settings(&cfg(json!({ "focus_tint": v }))).focus_tint;
    assert_eq!(tint(json!(0.3)), 0.3);
    assert_eq!(tint(json!("0.5")), 0.5);
    assert_eq!(tint(json!(true)), 1.0);
    assert_eq!(tint(json!(7)), 1.0);
    assert_eq!(tint(json!(-1)), 0.0);
    assert_eq!(tint(json!("lots")), DEFAULT_TINT);
    // not finite: the default (the stated deviation)
    assert_eq!(tint(json!("nan")), DEFAULT_TINT);
    assert_eq!(tint(json!(null)), DEFAULT_TINT);
    assert_eq!(tint(json!([1])), DEFAULT_TINT);
}

#[test]
fn own_backgrounds_must_be_colours() {
    let s = panel_settings(&cfg(json!({"panel_bg": "#000000", "focus_bg": "reset"})));
    assert_eq!((s.panel_bg, s.focus_bg), (Some(Rgb::hex(0)), None));
    assert_eq!(
        panel_settings(&cfg(json!({"panel_bg": "dark"}))).panel_bg,
        None
    );
}

#[test]
fn animation_fps_is_clamped_and_needs_a_number() {
    let fps = |v: Value| animation_fps(&cfg(json!({ "animation_fps": v })));
    assert_eq!(animation_fps(&cfg(json!({}))), 20.0);
    assert_eq!(fps(json!(60)), 30.0);
    assert_eq!(fps(json!(1)), 2.0);
    assert_eq!(fps(json!(12.5)), 12.5);
    assert_eq!(fps(json!(true)), 20.0);
    assert_eq!(fps(json!("30")), 20.0);
}
