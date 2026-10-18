use super::*;

/// The tool as the Python server lists it (`tests/fixtures/mcp/legacy.jsonl`, `tools/list`).
fn captured() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mcp/legacy.jsonl"
    ))
    .unwrap();
    let reply = text
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["dir"] == "server->client" && r["msg"]["id"] == 1)
        .unwrap();
    reply["msg"]["result"]["tools"][0].clone()
}

#[test]
fn the_tool_is_listed_word_for_word_as_the_python_server_lists_it() {
    assert_eq!(serde_json::to_value(tool()).unwrap(), captured());
}
