use super::*;

#[test]
fn the_launcher_asks_for_this_contract() {
    let script = include_str!("../../scripts/panel");
    let line = script
        .lines()
        .find(|l| l.starts_with("CONTRACT="))
        .expect("scripts/panel names its contract");
    assert_eq!(line, format!("CONTRACT={PANEL}"));
}

#[test]
fn the_contract_covers_this_protocol_major() {
    // a new protocol major is a new contract: bump both together
    assert_eq!(crate::proto::PROTOCOL.split('.').next(), Some("1"));
}
