use super::*;

const CODES: [&str; 12] = [
    "q", "run", "rev", "wait", "blk", "fail", "done", "cxl", "inf", "lost", "bg", "stale",
];

#[test]
fn every_code_reads_back_as_itself() {
    for code in CODES {
        let st = Status::from_code(code);
        assert!(!matches!(st, Status::Unknown(_)), "{code}");
        assert_eq!(st.code(), code);
    }
    assert_eq!(
        Status::from_code("new-kind"),
        Status::Unknown("new-kind".into())
    );
    assert_eq!(Status::from_code("new-kind").code(), "new-kind");
}

#[test]
fn glyphs_and_labels_are_pythons_tables() {
    // model.py GLYPH and LABEL, in CODES order
    let glyphs = ["○", "⠋", "»", "?", "!", "✗", "✓", "-", "✓?", "~", "⠁", "◌"];
    let labels = [
        "queued",
        "running",
        "reviewing",
        "needs answer",
        "blocked",
        "failed",
        "done",
        "cancelled",
        "done?",
        "lost",
        "waiting",
        "stale",
    ];
    for (i, code) in CODES.iter().enumerate() {
        let st = Status::from_code(code);
        assert_eq!(st.glyph(), glyphs[i], "{code}");
        assert_eq!(st.label(), labels[i], "{code}");
    }
    assert_eq!(Status::from_code("x").glyph(), "?");
}

#[test]
fn the_spinner_and_clock_start_on_the_still_glyphs() {
    assert_eq!(SPIN[0].to_string(), Status::Running.glyph());
    assert_eq!(CLOCK[0].to_string(), Status::Background.glyph());
}

#[test]
fn urgency_follows_pythons_order_and_unknown_comes_last() {
    let order = [
        "fail", "blk", "wait", "rev", "run", "bg", "q", "lost", "inf", "done", "cxl",
    ];
    for (i, code) in order.iter().enumerate() {
        assert_eq!(Status::from_code(code).urgency(), i, "{code}");
    }
    assert_eq!(Status::Stale.urgency(), 11);
    assert_eq!(Status::from_code("x").urgency(), 11);
}

#[test]
fn live_and_needs() {
    let live: Vec<&str> = CODES
        .into_iter()
        .filter(|c| Status::from_code(c).is_live())
        .collect();
    assert_eq!(
        live,
        ["q", "run", "rev", "wait", "blk", "fail", "bg", "stale"]
    );
    assert!(Status::from_code("x").is_live());
    let needs: Vec<&str> = CODES
        .into_iter()
        .filter(|c| Status::from_code(c).needs_operator())
        .collect();
    assert_eq!(needs, ["wait", "blk", "fail"]);
}

#[test]
fn deserializes_from_the_wire_code() {
    let st: Vec<Status> = serde_json::from_str(r#"["run", "zzz"]"#).unwrap();
    assert_eq!(st, [Status::Running, Status::Unknown("zzz".into())]);
}
