use super::*;

#[test]
fn only_the_same_major_is_compatible() {
    assert_eq!(major("1.3"), Some(1));
    assert!(compatible("1.0") && compatible("1.9") && compatible("1"));
    assert!(!compatible("2.0") && !compatible("x.1") && !compatible(""));
}
