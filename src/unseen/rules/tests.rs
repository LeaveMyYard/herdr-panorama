use super::*;

#[test]
fn events_and_urgency() {
    let st = Status::from_code;
    assert_eq!(event(Some("run"), &[], &st("fail"), &[]), Some(Event::Fail));
    assert_eq!(event(Some("fail"), &[], &st("fail"), &[]), None);
    assert_eq!(
        event(Some("run"), &[1], &st("wait"), &[1, 2]),
        Some(Event::Wait)
    );
    assert_eq!(event(Some("run"), &[], &st("inf"), &[]), Some(Event::Done));
    assert_eq!(event(Some("cxl"), &[], &st("done"), &[]), None);
    assert_eq!(event(Some("inf"), &[], &st("done"), &[]), None);
    assert_eq!(event(None, &[], &st("fail"), &[3]), Some(Event::Fail)); // fail beats wait
    assert_eq!(
        urgent(Some(Event::Done), Some(Event::Wait)),
        Some(Event::Wait)
    );
    assert_eq!(urgent(None, Some(Event::Done)), Some(Event::Done));
    assert_eq!(float(Some(&serde_json::json!("12.5"))), 12.5);
    assert_eq!(float(None), 0.0);
}
