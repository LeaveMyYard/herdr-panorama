use super::*;

#[test]
fn models_read_as_python_short_model() {
    let m = |s: &str| short_model(Some(s));
    assert_eq!(m("claude-opus-4-5-20251101").as_deref(), Some("opus 4.5"));
    assert_eq!(
        m("claude-3-5-sonnet-20241022").as_deref(),
        Some("sonnet 3.5")
    );
    assert_eq!(m("claude-sonnet-4-20250514").as_deref(), Some("sonnet 4"));
    assert_eq!(m("claude-haiku-4-5").as_deref(), Some("haiku 4.5"));
    assert_eq!(m("Opus").as_deref(), Some("opus"));
    assert_eq!(m("claude-3-opus").as_deref(), Some("opus 3"));
    assert_eq!(m("gpt-5").as_deref(), Some("gpt-5"));
    assert_eq!(m("fable"), Some("fable".into()));
    assert_eq!(short_model(Some("")), None);
    assert_eq!(short_model(None), None);
}

#[test]
fn paragraphs_are_the_first_one_collapsed() {
    assert_eq!(paragraph("  a  b\nc\n\nnext"), "a b c");
    let long = "x".repeat(500);
    let p = paragraph(&long);
    assert_eq!(p.chars().count(), 400);
    assert!(p.ends_with('…'));
}

#[test]
fn the_inbox_mark_is_stripped() {
    assert_eq!(strip_inbox_mark("❓3 work"), "work");
    assert_eq!(strip_inbox_mark("❓3 ❓12  work"), "work");
    assert_eq!(strip_inbox_mark("❓ work"), "❓ work");
    assert_eq!(strip_inbox_mark("❓3work"), "❓3work");
    assert_eq!(strip_inbox_mark("work ❓3 "), "work ❓3 ");
    assert_eq!(strip_inbox_mark("❓3 "), "");
}

#[test]
fn a_non_ascii_model_never_panics_and_reads_as_python_reads_it() {
    let m = |s: &str| short_model(Some(s));
    assert_eq!(m("claude-ö-opus-4").as_deref(), Some("opus 4"));
    assert_eq!(m("модель").as_deref(), Some("модель"));
    assert_eq!(m("xésonnet").as_deref(), Some("sonnet"));
    assert_eq!(m("ö4-5-haiku").as_deref(), Some("haiku 4.5"));
}
