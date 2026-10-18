use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn ok(v: &[&str]) -> Args {
    match parse(&args(v)).unwrap() {
        Parsed::Emit(a) => *a,
        Parsed::Help => panic!("help"),
    }
}

fn last_line(v: &[&str]) -> String {
    parse(&args(v))
        .unwrap_err()
        .lines()
        .last()
        .unwrap()
        .to_owned()
}

#[test]
fn flags_take_values_both_ways_and_repeat_where_they_should() {
    let a = ok(&[
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
    ]);
    assert_eq!(
        (a.thread.as_str(), a.name.as_deref(), a.pct),
        ("t", Some("M"), Some(40))
    );
    assert_eq!(a.phases, ["a", "b=now"]);
    assert_eq!(a.agents, ["x"]);
    assert_eq!(ok(&["--thread", "t", "--pct", "-5"]).pct, Some(-5)); // a negative number is a value
    assert_eq!(ok(&["--thread", "t", "--pct", "5_0"]).pct, Some(50)); // int() takes underscores
}

#[test]
fn a_unique_prefix_names_its_option() {
    let a = ok(&["--thr", "t", "--sess", "s", "--wor=w", "--st", "done"]);
    assert_eq!(
        (
            a.thread.as_str(),
            a.session.as_deref(),
            a.worktree.as_deref(),
            a.status.as_deref()
        ),
        ("t", Some("s"), Some("w"), Some("done"))
    );
    assert_eq!(
        last_line(&["--s", "x", "--thread", "t"]),
        "panorama emit: error: ambiguous option: --s could match --session, --status"
    );
}

#[test]
fn help_is_argparse_s_and_wins_where_it_is_met() {
    for v in [
        &["-h"][..],
        &["--thread", "t", "--help"],
        &["--he"],
        &["--name", "x", "-h"],
    ] {
        assert_eq!(parse(&args(v)), Ok(Parsed::Help), "{v:?}");
    }
    assert!(HELP.starts_with("usage: panorama emit [-h] [--session SESSION] --thread THREAD"));
    // an error met before it still wins, as in argparse
    assert!(parse(&args(&["--thread", "t", "--status", "x", "-h"])).is_err());
}

#[test]
fn errors_read_as_argparse_s() {
    let full = parse(&args(&["--name", "x"])).unwrap_err();
    assert!(full.starts_with("usage: panorama emit [-h]"), "{full}");
    assert!(full.ends_with("panorama emit: error: the following arguments are required: --thread"));
    assert_eq!(
        last_line(&["--thread"]),
        "panorama emit: error: argument --thread: expected one argument"
    );
    assert_eq!(
        last_line(&["--thread", "t", "--name", "-dash"]),
        "panorama emit: error: argument --name: expected one argument"
    );
    assert_eq!(
        last_line(&["--thread", "t", "--status", "maybe"]),
        "panorama emit: error: argument --status: invalid choice: 'maybe' (choose from active, done, failed, cancelled)"
    );
    assert_eq!(
        last_line(&["--thread", "t", "--pct", "x"]),
        "panorama emit: error: argument --pct: invalid int value: 'x'"
    );
    let extra = parse(&args(&["--thread", "t", "--bogus", "1", "x"])).unwrap_err();
    assert!(extra.starts_with("usage: panorama [-h]"));
    assert!(extra.ends_with("panorama: error: unrecognized arguments: --bogus 1 x"));
}
