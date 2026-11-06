use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_format_comes_from_the_flags() {
    assert_eq!(Format::from_args(&args(&[])), Some(Format::Text));
    assert_eq!(Format::from_args(&args(&["--json"])), Some(Format::Json));
    assert_eq!(
        Format::from_args(&args(&["--json", "--counts"])),
        Some(Format::Counts)
    );
    assert_eq!(
        Format::from_args(&args(&["--counts", "--json"])),
        Some(Format::Counts)
    );
    assert_eq!(Format::from_args(&args(&["--yaml"])), None);
}

#[test]
fn an_empty_swarm_prints_its_totals_and_no_roots() {
    let swarm = Swarm {
        generated_at: 1.5,
        ..Swarm::default()
    };
    let out = render(&swarm, Format::Json, 0.0);
    assert!(out.starts_with("{\n  \"generated_at\": 1.5,\n  \"herdr_ok\": true,\n  \"inbox_ok\": false,\n  \"errors\": [],\n  \"counts\": {\n    \"roots\": 0,\n    \"agents\": 0,"), "{out}");
    assert!(out.ends_with("  \"roots\": []\n}\n"), "{out}");
    assert_eq!(render(&swarm, Format::Text, 0.0), "");
    let counts = render(&swarm, Format::Counts, 0.12345);
    assert!(
        counts.contains("\"agent_status\": {},")
            && counts.contains("\"hidden_finished\": 0,")
            && counts.contains("\"seconds\": 0.123\n"),
        "{counts}"
    );
}
