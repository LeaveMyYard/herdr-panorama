//! Bounded newest-first history, with verified parent links. Never scans by cwd or recency.
use super::{
    Observation, Options, Thread, hydrate, parse,
    protocol::{Client, Error, Result},
};
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
const MAX_THREADS: usize = 8;
use super::hydrate::MAX_TURNS;
const ITEM_PAGE: usize = 64;
const ITEM_PAGES: usize = 2;

async fn read(
    client: &mut Client,
    id: &str,
    unsupported: &mut HashSet<String>,
) -> Result<(Thread, Vec<String>, bool)> {
    let meta = client
        .call("thread/read", json!({"threadId":id,"includeTurns":false}))
        .await?;
    let raw = meta.get("thread").ok_or(Error::Invalid)?;
    let mut thread = parse::thread(raw, id).ok_or(Error::Invalid)?;
    let mode = match raw.get("historyMode").and_then(Value::as_str) {
        None | Some("legacy") => "legacy",
        Some("paginated") => "paginated",
        Some(_) => "other",
    };
    if unsupported.contains(mode) {
        return hydrate::full_turns(client, thread).await;
    }
    let turns = client
        .call(
            "thread/turns/list",
            json!({"threadId":id,"limit":MAX_TURNS,"sortDirection":"desc","itemsView":"notLoaded"}),
        )
        .await;
    if turns == Err(Error::Unsupported) && mode == "legacy" {
        return hydrate::legacy(client, thread).await;
    }
    let turns = turns?;
    let data = turns
        .get("data")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid)?;
    thread.turns = data
        .iter()
        .take(MAX_TURNS)
        .filter_map(parse::turn)
        .collect();
    let mut partial = data.len() > MAX_TURNS || cursor(&turns).is_some();
    let mut children = Vec::new();
    let mut next: Option<String> = None;
    let mut cursors = HashSet::new();
    let mut seen_items = HashSet::new();
    for _ in 0..ITEM_PAGES {
        let page = client
            .call(
                "thread/items/list",
                json!({"threadId":id,"limit":ITEM_PAGE,"sortDirection":"desc","cursor":next}),
            )
            .await;
        if page == Err(Error::Unsupported) {
            unsupported.insert(mode.to_owned());
            return hydrate::full_turns(client, thread).await;
        }
        let Ok(page) = page else {
            partial = true;
            break;
        };
        let Some(data) = page.get("data").and_then(Value::as_array) else {
            partial = true;
            break;
        };
        partial |= data.len() > ITEM_PAGE;
        for entry in data.iter().take(ITEM_PAGE) {
            if let Some(id) = entry.get("item").and_then(|v| parse::id(v, "id"))
                && seen_items.insert(id)
            {
                parse::item(&mut thread, entry, &mut children);
            }
        }
        next = cursor(&page);
        if next.is_none() {
            break;
        }
        if !cursors.insert(next.clone()) {
            partial = true;
            break;
        }
    }
    partial |= next.is_some();
    children.sort();
    children.dedup();
    Ok((thread, children, partial))
}
fn cursor(v: &Value) -> Option<String> {
    v.get("nextCursor")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
async fn query(
    client: &mut Client,
    id: &str,
    unsupported: &mut HashSet<String>,
    deadline: tokio::time::Instant,
) -> Result<Observation> {
    let (root, child_ids, mut partial) = tokio::time::timeout_at(deadline, async {
        client.initialize().await?;
        read(client, id, unsupported).await
    })
    .await
    .unwrap_or(Err(Error::Unavailable))?;
    let mut queue: VecDeque<_> = child_ids
        .into_iter()
        .map(|child| (id.to_owned(), child))
        .collect();
    let mut seen = HashSet::from([id.to_owned()]);
    let mut children = Vec::new();
    let mut reads = 1;
    while let Some((parent, child)) = queue.pop_front() {
        if seen.contains(&child) {
            continue;
        }
        if reads >= MAX_THREADS {
            partial = true;
            break;
        }
        reads += 1;
        let child_result =
            match tokio::time::timeout_at(deadline, read(client, &child, unsupported)).await {
                Ok(result) => result,
                Err(_) => {
                    partial = true;
                    break;
                }
            };
        match child_result {
            Ok((thread, ids, truncated)) if thread.parent_thread_id.as_deref() == Some(&parent) => {
                seen.insert(child.clone());
                partial |= truncated;
                queue.extend(ids.into_iter().map(|id| (child.clone(), id)));
                children.push(thread);
            }
            _ => partial = true,
        }
    }
    Ok(Observation {
        root,
        children,
        partial,
        observed_at: crate::snapshot::now(),
    })
}
/// Always kills and reaps the process that this call created, including on timeout.
pub(super) async fn fetch_cancellable(
    options: &Options,
    id: &str,
    unsupported: &mut HashSet<String>,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Observation> {
    let mut client = Client::spawn(options)?;
    let deadline =
        tokio::time::Instant::now() + options.timeout.min(std::time::Duration::from_secs(10));
    let result = tokio::select! {
        result = query(&mut client, id, unsupported, deadline) => result,
        _ = async {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        } => Err(Error::Unavailable),
    };
    client.stop().await;
    result
}

#[cfg(test)]
pub(super) async fn fetch(options: &Options, id: &str) -> Result<Observation> {
    fetch_with(options, id, &mut HashSet::new()).await
}

#[cfg(test)]
pub(super) async fn fetch_with(
    options: &Options,
    id: &str,
    unsupported: &mut HashSet<String>,
) -> Result<Observation> {
    fetch_cancellable(
        options,
        id,
        unsupported,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .await
}
