use super::*;

#[test]
fn the_earliest_role_word_wins() {
    assert_eq!(role_of(&["Review PR #42"], None), "reviewer");
    assert_eq!(role_of(&["fix the review comments"], None), "fixer");
    assert_eq!(role_of(&["", "implement the API"], None), "author");
    assert_eq!(role_of(&["prefix only"], None), "agent"); // "fix" inside a word does not count
    assert_eq!(
        role_of(&["hello"], Some("general-purpose")),
        "general-purpose"
    );
    assert_eq!(phase_word("reviewer"), "review");
    assert_eq!(phase_word("writer"), "writer");
}

#[test]
fn references_name_their_repository_unless_it_is_a_stop_word() {
    assert_eq!(ref_key("Review PR #42"), Some(("#42".into(), "#42".into())));
    assert_eq!(
        ref_key("herdr-inbox #7 fix"),
        Some(("herdr-inbox#7".into(), "herdr-inbox #7".into()))
    );
    assert_eq!(
        ref_key("see pull/12"),
        Some(("see#12".into(), "see #12".into()))
    );
    assert_eq!(ref_key("pull/12"), Some(("#12".into(), "#12".into())));
    assert_eq!(ref_key("Fix PR 3"), Some(("#3".into(), "#3".into())));
    assert_eq!(ref_key("no reference"), None);
    assert_eq!(ref_key("#1234567"), None);
}
