use super::*;
use serde_json::json;

#[test]
fn truth_and_str_read_as_python() {
    for (v, t) in [
        (json!(null), false),
        (json!(0), false),
        (json!(0.0), false),
        (json!(""), false),
        (json!([]), false),
        (json!({}), false),
        (json!(1), true),
        (json!("x"), true),
        (json!([0]), true),
    ] {
        assert_eq!(truthy(&v), t, "{v}");
    }
    assert_eq!(py_str(&json!(3)), "3");
    assert_eq!(py_str(&json!(1.0)), "1.0");
    assert_eq!(py_str(&json!(true)), "True");
    assert_eq!(py_str(&json!(null)), "None");
}

#[test]
fn int_and_times_read_as_python() {
    assert_eq!(py_int(Some(&json!("2")), 1), 2);
    assert_eq!(py_int(Some(&json!(" 1_0 ")), 1), 10);
    assert_eq!(py_int(Some(&json!(2.9)), 1), 2);
    assert_eq!(py_int(Some(&json!(true)), 5), 1);
    assert_eq!(py_int(Some(&json!("x")), 1), 1);
    assert_eq!(py_int(None, 1), 1);
    assert_eq!(or_time(Some(0.0), Some(2.0)), Some(2.0));
    assert_eq!(or_time(Some(1.0), Some(2.0)), Some(1.0));
    assert_eq!(truthy_time(Some(0.0)), None);
}

#[test]
fn splitlines_reads_as_python() {
    assert_eq!(
        py_splitlines("a\r\nb\rc\nd\u{2028}e"),
        ["a", "b", "c", "d", "e"]
    );
    assert_eq!(py_splitlines("a\n"), ["a"]);
    assert!(py_splitlines("").is_empty());
    assert_eq!(chars("héllo", 2), "hé");
    assert!(py_isspace('\x1c') && py_isspace(' ') && !py_isspace('x'));
}
