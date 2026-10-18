use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn flags_take_values_both_ways_and_repeat_where_they_should() {
    let a = parse(&args(&[
        "--thread",
        "t",
        "--name=N",
        "--phase",
        "a",
        "--phase=b=now",
        "--agent",
        "x",
        "--pct",
        " 40 ",
        "--name",
        "M",
    ]))
    .unwrap();
    assert_eq!(
        (a.thread.as_str(), a.name.as_deref(), a.pct),
        ("t", Some("M"), Some(40))
    );
    assert_eq!(a.phases, ["a", "b=now"]);
    assert_eq!(a.agents, ["x"]);
}

#[test]
fn what_argparse_refuses_is_refused() {
    for bad in [
        &["--name", "x"][..],
        &["--thread"],
        &["--thread", "t", "--status", "maybe"],
        &["--thread", "t", "--pct", "x"],
        &["--thread", "t", "--bogus", "1"],
        &["stray"],
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?}");
    }
    assert_eq!(
        parse(&args(&["--thread", "t", "--status", "done"]))
            .unwrap()
            .status
            .as_deref(),
        Some("done")
    );
}
