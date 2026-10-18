use super::*;

pub(crate) const SID: &str = "11111111-2222-3333-4444-555555555555";

#[test]
fn malformed_session_ids_are_not_sessions() {
    assert!(valid_session(SID));
    assert!(valid_session(&SID.to_uppercase()));
    let long = "x".repeat(100);
    let newline = format!("{SID}\n");
    for bad in [
        "",
        ".",
        "..",
        "../x",
        "a b",
        "--------",
        long.as_str(),
        "zzzzzzzz-zzzz",
        newline.as_str(),
    ] {
        assert!(!valid_session(bad), "{bad:?}");
    }
}

#[test]
fn an_ssh_target_that_reads_as_an_option_or_has_spaces_is_unsafe() {
    assert!(safe_target("devbox"));
    assert!(safe_target("me@host:2222"));
    for bad in ["", "-oProxyCommand=x", "dev box", "dev\tbox", "a\n"] {
        assert!(!safe_target(bad), "{bad:?}");
    }
}

#[test]
fn errors_are_cut_by_characters() {
    assert_eq!(cut("é".repeat(250)).chars().count(), ERROR_CHARS);
    assert_eq!(cut("short".into()), "short");
}
