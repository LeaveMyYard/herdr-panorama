//! Compatibility hydration when item pagination is not implemented (Codex 0.166).
use super::{
    Thread,
    discovery::{self, Scan},
    parse,
    protocol::{Client, Error, Result},
};
use serde_json::{Value, json};
pub(super) const MAX_TURNS: usize = 16;
const MAX_ITEMS: usize = 128;

pub(super) async fn full_turns(
    client: &mut Client,
    thread: Thread,
    scan: &mut Scan,
) -> Result<(Thread, Vec<String>, bool)> {
    let response = client
        .call(
            "thread/turns/list",
            json!({"threadId":thread.thread_id,
        "limit":MAX_TURNS,"sortDirection":"desc","itemsView":"full"}),
        )
        .await?;
    let turns = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid)?;
    let partial = response.get("nextCursor").is_some_and(|v| !v.is_null());
    let (thread, ids, partial) = fold(thread, turns.iter(), turns.len() > MAX_TURNS || partial);
    scan.remember(ids);
    let incomplete = discovery::turns(client, &thread.thread_id, scan).await;
    Ok((thread, scan.candidates(), partial || incomplete))
}
/// Only legacy threads on older servers may use full-history thread/read. Wire
/// size and whole-refresh deadline limits still apply; no private SQLite parsing.
pub(super) async fn legacy(
    client: &mut Client,
    thread: Thread,
    scan: &mut Scan,
) -> Result<(Thread, Vec<String>, bool)> {
    let response = client
        .call(
            "thread/read",
            json!({"threadId":thread.thread_id,"includeTurns":true}),
        )
        .await?;
    let raw = response.get("thread").ok_or(Error::Invalid)?;
    parse::thread(raw, &thread.thread_id).ok_or(Error::Invalid)?;
    let turns = raw
        .get("turns")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid)?;
    let (thread, ids, partial) = fold(thread, turns.iter().rev(), turns.len() > MAX_TURNS);
    scan.remember(ids);
    scan.legacy(turns);
    Ok((thread, scan.candidates(), partial || scan.limited()))
}
fn fold<'a>(
    mut thread: Thread,
    turns: impl Iterator<Item = &'a Value>,
    mut partial: bool,
) -> (Thread, Vec<String>, bool) {
    thread.turns.clear();
    thread.latest_tool = None;
    let mut children = Vec::new();
    let mut remaining = MAX_ITEMS;
    for value in turns.take(MAX_TURNS) {
        let Some(turn) = parse::turn(value) else {
            partial = true;
            continue;
        };
        let turn_id = turn.id.clone();
        thread.turns.push(turn);
        if value
            .get("itemsView")
            .and_then(Value::as_str)
            .is_some_and(|s| s != "full")
        {
            partial = true;
        }
        for item in value
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .rev()
        {
            if remaining == 0 {
                partial = true;
                break;
            }
            remaining -= 1;
            parse::item(
                &mut thread,
                &json!({"item":item,"turnId":turn_id}),
                &mut children,
            );
        }
    }
    children.sort();
    children.dedup();
    (thread, children, partial)
}
