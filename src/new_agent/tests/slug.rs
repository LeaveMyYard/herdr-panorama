//! herdr's agent names from free text (Python `tests/test_home.py::test_slugify`).

use crate::new_agent::{name_ok, slugify};

#[test]
fn slugs_from_free_text() {
    for (name, slug) in [
        ("Billing API!", "billing-api"),
        ("  9 lives ", "a-9-lives"),
        (&"x".repeat(40)[..], &"x".repeat(32)[..]),
        ("Ünïcödé", "n-c-d"),
        ("!!!", ""),
        ("fix_auth--flow", "fix_auth-flow"),
    ] {
        assert_eq!(slugify(name), slug, "{name:?}");
        assert!(slug.is_empty() || name_ok(slug), "{slug}");
    }
}

#[test]
fn the_name_rule_is_herdrs() {
    assert!(name_ok("a") && name_ok("billing-api") && name_ok("a_9-x") && name_ok(&"x".repeat(32)));
    assert!(
        !name_ok("")
            && !name_ok("9a")
            && !name_ok("-a")
            && !name_ok("A")
            && !name_ok(&"x".repeat(33))
    );
    assert!(!name_ok("a b") && !name_ok("a.b"));
    // every slug is a name, or empty; a name cut at 32 never ends in - or _
    assert_eq!(slugify(&format!("{}-x", "y".repeat(31))), "y".repeat(31));
}
