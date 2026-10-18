use super::*;

#[test]
fn known_words_read_back_and_others_are_kept() {
    for w in ["working", "blocked", "idle", "done", "unknown"] {
        assert!(!matches!(HerdrStatus::from_code(w), HerdrStatus::Other(_)));
        assert_eq!(HerdrStatus::from_code(w).code(), w);
    }
    for w in ["active", "done", "failed", "cancelled"] {
        assert_eq!(ThreadStatus::from_code(w).code(), w);
    }
    for w in ["explicit", "workflow", "ref", "turn", "session"] {
        assert_eq!(ThreadSource::from_code(w).code(), w);
    }
    for w in ["done", "now", "next", "failed", "skipped"] {
        assert_eq!(PhaseState::from_code(w).code(), w);
    }
    assert_eq!(
        HerdrStatus::from_code("sleeping"),
        HerdrStatus::Other("sleeping".into())
    );
    assert_eq!(ThreadSource::from_code("x").code(), "x");
}

#[test]
fn defaults_are_model_pys() {
    assert_eq!(HerdrStatus::default(), HerdrStatus::Unknown);
    assert_eq!(ThreadStatus::default(), ThreadStatus::Active);
}
