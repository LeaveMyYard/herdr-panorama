use super::*;

#[test]
fn nonempty_core_address_precedes_tui_and_empty_values_are_absent() {
    for (vars, host) in [
        (vec![], Host::Unknown),
        (
            vec![("HC_TERMINAL_ID", ""), ("HERDR_PANE_ID", "")],
            Host::Unknown,
        ),
        (vec![("HERDR_PANE_ID", "w1:p1")], Host::Tui),
        (
            vec![("HC_TERMINAL_ID", ""), ("HERDR_PANE_ID", "w1:p1")],
            Host::Tui,
        ),
        (vec![("HC_TERMINAL_ID", "t2")], Host::Core),
        (
            vec![("HC_TERMINAL_ID", "vps/t7"), ("HERDR_PANE_ID", "w1:p1")],
            Host::Core,
        ),
    ] {
        assert_eq!(
            Context::from_env(&PluginEnv::from_vars(vars, None, 1000)).host,
            host
        );
    }
}

#[test]
fn addresses_preserve_qualification_and_refuse_names_or_fragments() {
    for address in ["t2", "t1b", "vps/t7", "box-2/tabc"] {
        assert!(valid_address(address));
    }
    for address in [
        "",
        "t",
        "x7",
        "vps/",
        "/t7",
        "vps/t7/x",
        "t7 --kill",
        "$(whoami)",
    ] {
        assert!(!valid_address(address));
    }
    assert_ne!(pane_key("t7"), pane_key("vps/t7"));
}
