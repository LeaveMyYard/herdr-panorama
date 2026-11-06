use super::*;

#[test]
fn only_the_same_major_is_compatible() {
    assert_eq!(major("1.3"), Some(1));
    assert!(compatible("2.0") && compatible("2.9") && compatible("2"));
    assert!(!compatible("1.4") && !compatible("3.0") && !compatible("x.1") && !compatible(""));
}

#[test]
fn an_open_key_may_still_say_one() {
    assert!(key_compatible("1.3") && key_compatible("1") && key_compatible("2.0"));
    assert!(!key_compatible("0.9") && !key_compatible("3.0") && !key_compatible(""));
}
