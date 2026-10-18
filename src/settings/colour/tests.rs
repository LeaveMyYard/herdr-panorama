use super::*;
use serde_json::json;

#[test]
fn colour_values_read_as_theme_py_reads_them() {
    let c = |v: Value| colour(&v);
    assert_eq!(c(json!("#1A1b26")), ColourValue::Colour(Rgb::hex(0x1a1b26)));
    assert_eq!(
        c(json!(" rgb( 26, 27,38 ) ")),
        ColourValue::Colour(Rgb::hex(0x1a1b26))
    );
    assert_eq!(c(json!("Grey")), ColourValue::Colour(Rgb::hex(0x808080)));
    for reset in ["reset", "Default", " none ", "transparent"] {
        assert_eq!(c(json!(reset)), ColourValue::Reset, "{reset}");
    }
    for bad in [
        json!("rgb(256, 0, 0)"),
        json!("rgb(0001,2,3)"),
        json!("rgb(1,2)"),
        json!("#12345"),
        json!("orange"),
        json!(5),
        json!(null),
    ] {
        assert_eq!(c(bad.clone()), ColourValue::Invalid, "{bad}");
    }
}
