use super::*;

#[test]
fn a_file_that_is_not_the_format_reads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("u.json");
    assert!(read(&p).is_none());
    for text in ["{not json", "[]", r#"{"agents": []}"#] {
        std::fs::write(&p, text).unwrap();
        assert!(read(&p).is_none(), "{text}");
    }
    std::fs::write(&p, r#"{"v": 1, "agents": {}, "extra": 3}"#).unwrap();
    let data = read(&p).unwrap();
    write(&p, &data).unwrap();
    assert_eq!(read(&p).unwrap().get("extra"), Some(&J::Int(3))); // what it does not know is kept
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        r#"{"v":1,"agents":{},"extra":3}"#
    ); // in order
}
