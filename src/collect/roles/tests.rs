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
}
