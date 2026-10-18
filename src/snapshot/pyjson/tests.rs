use super::*;

#[test]
fn floats_read_as_python_repr() {
    for (f, want) in [
        (1791969600.0, "1791969600.0"),
        (1791972540.009, "1791972540.009"),
        (0.0, "0.0"),
        (-1.5, "-1.5"),
        (1e16, "1e+16"),
        (1.5e16, "1.5e+16"),
        (1234567890123456.0, "1234567890123456.0"),
        (0.0001, "0.0001"),
        (0.00001, "1e-05"),
        (0.1, "0.1"),
    ] {
        assert_eq!(float_repr(f), want, "{f}");
    }
}

#[test]
fn layout_matches_python_indent() {
    let j = J::obj([
        ("a", J::Int(1)),
        ("b", J::Arr(vec![J::Float(1.0), J::Null])),
        ("c", J::Arr(vec![])),
        ("d", J::Obj(vec![])),
        ("e", J::str("x\"y\n\u{1}…")),
    ]);
    assert_eq!(
        dumps(&j, 1),
        "{\n \"a\": 1,\n \"b\": [\n  1.0,\n  null\n ],\n \"c\": [],\n \"d\": {},\n \"e\": \"x\\\"y\\n\\u0001…\"\n}"
    );
}

#[test]
fn control_characters_are_escaped_and_del_is_not() {
    let j = J::str("a\u{1}b\u{1b}[2Jc\u{1f}d\u{7f}e");
    assert_eq!(dumps(&j, 1), "\"a\\u0001b\\u001b[2Jc\\u001fd\u{7f}e\"");
}

#[test]
fn compact_ascii_lines_and_ordered_reads_round_trip_as_python_writes() {
    let text = r#"{"v":1,"saved":1791972660.25,"agents":{"z":{"s":"fail","q":[3],"ev":null,"t":2.0},"m\u00e4:x":{"s":"inf"}}}"#;
    let j = J::parse(text).unwrap();
    let style = Style {
        item: ",",
        key: ":",
        ascii: true,
    };
    assert_eq!(dumps_styled(&j, style), text);
    assert_eq!(
        j.get("agents")
            .and_then(|a| a.get("mä:x"))
            .and_then(|r| r.get("s"))
            .and_then(J::as_str),
        Some("inf")
    );
    let mut j = j;
    j.set("saved", J::Float(5.0));
    j.set("new", J::Null);
    assert!(dumps_styled(&j, style).starts_with(r#"{"v":1,"saved":5.0,"agents":"#));
    assert!(dumps_styled(&j, style).ends_with(r#","new":null}"#));
    assert_eq!(
        J::parse(r#"{"a":1,"a":2}"#),
        Some(J::obj([("a", J::Int(2))]))
    );
    assert_eq!(
        dumps_styled(
            &J::str("\u{7f}é"),
            Style {
                item: ", ",
                key: ": ",
                ascii: true
            }
        ),
        r#""\u007f\u00e9""#
    );
}

#[test]
fn one_line_matches_python_separators() {
    let j = J::obj([
        ("a", J::Int(1)),
        ("b", J::Arr(vec![J::Float(1.5), J::Null, J::Obj(vec![])])),
        ("c", J::str("é")),
    ]);
    assert_eq!(
        dumps_line(&j),
        r#"{"a": 1, "b": [1.5, null, {}], "c": "é"}"#
    );
}

#[test]
fn ascii_lines_escape_as_python_does() {
    let j = J::obj([("a", J::str("é😀\"x"))]);
    assert_eq!(dumps_line_ascii(&j), r#"{"a": "\u00e9\ud83d\ude00\"x"}"#);
}

#[test]
fn ascii_lines_escape_del_as_python_does() {
    assert_eq!(
        dumps_line_ascii(&J::str("a\u{0}b\u{7f} ")),
        r#""a\u0000b\u007f ""#
    );
}
