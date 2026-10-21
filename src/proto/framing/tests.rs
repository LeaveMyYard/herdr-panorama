use super::*;
use serde_json::json;

#[test]
fn a_message_is_one_compact_line() {
    let line = encode(&json!({"seen": ["a", "é"]})).unwrap();
    assert_eq!(line, "{\"seen\":[\"a\",\"é\"]}\n".as_bytes());
}

#[test]
fn messages_split_across_reads_come_out_whole() {
    let mut r = LineReader::default();
    assert!(r.feed(b"{\"ping\":").unwrap().is_empty());
    let got = r.feed(b"1}\n{\"bye\":{}}\n{\"x\"").unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0]["ping"], 1);
    assert!(got[1].contains_key("bye"));
    assert_eq!(r.feed(b":2}\n").unwrap()[0]["x"], 2);
}

#[test]
fn lines_that_are_not_objects_are_skipped() {
    let mut r = LineReader::default();
    let got = r
        .feed(b"[1]\nnot json\n\"s\"\n\n\xff\xfe\n{\"ok\":true}\n")
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["ok"], true);
}

#[test]
fn a_line_over_the_limit_ends_the_connection() {
    let mut r = LineReader::default();
    let mut big = vec![b'x'; MAX_LINE + 1];
    big.push(b'\n');
    assert_eq!(r.feed(&big), Err(TooLong(MAX_LINE + 1)));
    // without a newline too, once the buffer outgrows the limit
    let mut r = LineReader::default();
    assert!(r.feed(&vec![b'x'; MAX_LINE]).unwrap().is_empty());
    assert_eq!(r.feed(b"y"), Err(TooLong(MAX_LINE + 1)));
    // exactly the limit is fine
    let mut r = LineReader::default();
    let mut ok = br#"{"a":""#.to_vec();
    ok.resize(MAX_LINE - 2, b'z');
    ok.extend_from_slice(b"\"}\n");
    assert_eq!(r.feed(&ok).unwrap().len(), 1);
    assert!(encode(&json!({"a": "z".repeat(MAX_LINE)})).is_err());
}

#[test]
fn a_feed_splits_its_lines_once() {
    // test_core.py::test_the_line_reader_is_linear_and_one_read_reports_seen_once: 3 MB of
    // small lines in one feed (about 6 s in Python when every line re-copied the rest)
    let many = b"{\"seen\":[\"a1\"]}\n".repeat(200_000);
    let t0 = std::time::Instant::now();
    assert_eq!(LineReader::default().feed(&many).unwrap().len(), 200_000);
    assert!(
        t0.elapsed() < std::time::Duration::from_millis(1500),
        "{:?}",
        t0.elapsed()
    );
}
