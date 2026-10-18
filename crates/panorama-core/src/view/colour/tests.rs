use super::*;

#[test]
fn hex_parse_and_display_round_trip() {
    let c = Rgb::hex(0x101215);
    assert_eq!(c, Rgb(0x10, 0x12, 0x15));
    assert_eq!(c.to_string(), "#101215");
    assert_eq!(Rgb::parse("#FFb547"), Some(Rgb::hex(0xffb547)));
    for bad in ["101215", "#10121", "#1012155", "#10121g", ""] {
        assert_eq!(Rgb::parse(bad), None, "{bad}");
    }
}

#[test]
fn mix_rounds_half_up_like_python() {
    // values checked against render.mix / theme._mix
    let a = Rgb::hex(0x101215);
    assert_eq!(a.mix(Rgb::hex(0xffffff), 0.04).to_string(), "#1a1b1e");
    assert_eq!(
        Rgb::hex(0x14161b).mix(Rgb::hex(0xffffff), 0.04).to_string(),
        "#1d1f24"
    );
    assert_eq!(a.mix(Rgb::hex(0xff6b6b), 0.0), a);
    assert_eq!(a.mix(Rgb::hex(0xff6b6b), 1.0), Rgb::hex(0xff6b6b));
    // halfway between 0 and 1 rounds up, between 1 and 0 too (floor(x + 0.5))
    assert_eq!(Rgb(0, 1, 0).mix(Rgb(1, 0, 0), 0.5), Rgb(1, 1, 0));
}
