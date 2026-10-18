use super::*;

fn read(v: Value) -> FromCore {
    FromCore::read(v.as_object().unwrap().clone())
}

#[test]
fn the_viewers_messages_are_the_protocols() {
    let hello = ToCore::Hello {
        pane_id: Some("w1:p9".into()),
        tab_id: None,
    }
    .to_json();
    assert_eq!(
        hello,
        json!({"hello": {"protocol": "1.3", "role": "panel", "pane_id": "w1:p9", "tab_id": null}})
    );
    assert_eq!(
        ToCore::Seen(vec!["a".into()]).to_json(),
        json!({"seen": ["a"]})
    );
    assert_eq!(
        ToCore::Focusing("p".into()).to_json(),
        json!({"focusing": {"pane_id": "p"}})
    );
    assert_eq!(
        ToCore::Filter("needs".into()).to_json(),
        json!({"filter": {"name": "needs"}})
    );
    assert_eq!(ToCore::Bye.to_json(), json!({"bye": {}}));
}

#[test]
fn the_cores_messages_are_told_by_their_key() {
    assert_eq!(
        read(json!({"welcome": {"protocol": "1.3"}})),
        FromCore::Welcome
    );
    assert_eq!(
        read(json!({"error": {"message": "protocol", "protocol": "2.0"}})),
        FromCore::Error("protocol".into())
    );
    assert_eq!(
        read(json!({"error": "x"})),
        FromCore::Error("refused".into())
    );
    assert_eq!(
        read(json!({"state": {"protocol": "1.3"}})),
        FromCore::State(json!({"protocol": "1.3"}).as_object().unwrap().clone())
    );
    assert_eq!(read(json!({"state": [1]})), FromCore::Other);
    assert_eq!(read(json!({"shutdown": {}})), FromCore::Shutdown);
    assert_eq!(read(json!({"ping": 3})), FromCore::Other);
    assert_eq!(
        read(json!({"opened": {"result": "panel"}})),
        FromCore::Other
    );
}
