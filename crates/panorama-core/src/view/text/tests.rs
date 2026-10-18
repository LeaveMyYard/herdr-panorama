use super::*;

#[test]
fn trunc_and_fit_count_characters() {
    assert_eq!(trunc("abcdef", 4), "abc…");
    assert_eq!(trunc("abcd", 4), "abcd");
    assert_eq!(trunc("✓✓✓", 2), "✓…");
    assert_eq!(trunc("abc", 0), "…");
    assert_eq!(trunc("abc", -3), "…");
    assert_eq!(trunc("", 0), "");
    assert_eq!(fit("ab", 4), "ab  ");
    assert_eq!(fit("abcde", 4), "abc…");
    assert_eq!(fit("ab", 0), "");
}

#[test]
fn durations_and_ages() {
    assert_eq!(dur(None), "0m");
    assert_eq!(dur(Some(-5.0)), "0m");
    assert_eq!(dur(Some(59.0 * 60.0 + 59.0)), "59m");
    assert_eq!(dur(Some(3900.0)), "1h05m");
    assert_eq!(ago(59.0 * 60.0), "59m");
    assert_eq!(ago(3600.0 * 47.0 + 59.0 * 60.0), "47h");
    assert_eq!(ago(2880.0 * 60.0), "2d");
}

#[test]
fn clock_times() {
    // 1791972240 is 10:04 UTC
    assert_eq!(hhmm(Some(1791972240.0), 0), "10:04");
    assert_eq!(hhmm(Some(1791972240.0), 3 * 3600), "13:04");
    assert_eq!(hhmm(Some(1791972240.0), -20 * 3600), "14:04");
    assert_eq!(hhmm(None, 0), "");
    assert_eq!(hhmm(Some(0.0), 0), "");
}
