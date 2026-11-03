//! Bounded ancestry progress, independent of the recent display window. Only candidate
//! IDs/cursors are cached: metadata and parent proof are read again for every observation.
use super::{parse, protocol::Client};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

pub(super) const MAX_THREADS: usize = 8;
const MAX_CANDIDATES: usize = 64;
mod graph;
pub(super) use graph::Graph;
#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Unknown,
    Items,
    Turns,
    Legacy,
}
const MAX_CURSOR: usize = 4096;
const MAX_CURSORS: usize = 128;
const ITEMS: usize = 256;

#[derive(Default)]
pub(super) struct Scan {
    children: BTreeSet<String>,
    cursor: Option<String>,
    offset: usize,
    cursors: HashSet<u64>,
    limited: bool,
    mode: Mode,
    rotation: usize,
}
impl Scan {
    fn mode(&mut self, mode: Mode) {
        if self.mode != mode {
            self.cursor = None;
            self.offset = 0;
            self.cursors.clear();
            self.mode = mode;
        }
    }
    pub fn limited(&self) -> bool {
        self.limited
    }

    pub fn remember(&mut self, ids: impl IntoIterator<Item = String>) {
        for id in ids {
            if self.children.len() < MAX_CANDIDATES || self.children.contains(&id) {
                self.children.insert(id);
            } else {
                self.limited = true;
            }
        }
    }
    pub fn candidates(&self) -> Vec<String> {
        self.children.iter().cloned().collect()
    }
    fn items<'a>(&mut self, values: impl Iterator<Item = &'a Value>) {
        let mut ids = Vec::new();
        for item in values {
            parse::child_ids(item, &mut ids);
        }
        self.remember(ids);
    }
    fn advance(&mut self, value: &Value) -> bool {
        if value
            .get("nextCursor")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        {
            self.limited = true;
            return false;
        }
        let cursor = value
            .get("nextCursor")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty());
        self.offset = 0;
        if let Some(cursor) = cursor {
            let mut hash = DefaultHasher::new();
            cursor.hash(&mut hash);
            if cursor.len() > MAX_CURSOR
                || self.cursors.len() >= MAX_CURSORS
                || !self.cursors.insert(hash.finish())
            {
                self.limited = true;
                self.cursor = None;
                self.cursors.clear();
                return false;
            }
        } else {
            self.cursors.clear();
        }
        self.cursor = cursor.map(str::to_owned);
        true
    }
    /// Legacy full reads already carry all turns. Scan an oldest-first bounded slice;
    /// append-only history preserves offsets; reaching the end starts another cycle.
    pub fn legacy(&mut self, turns: &[Value]) {
        self.mode(Mode::Legacy);
        let values: Vec<_> = turns
            .iter()
            .flat_map(|t| {
                t.get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .collect();
        if self.offset >= values.len() {
            self.offset = 0;
        }
        self.items(values.iter().skip(self.offset).take(ITEMS).copied());
        self.offset = (self.offset + ITEMS).min(values.len());
        if self.offset == values.len() {
            self.offset = 0;
        }
    }
}

/// Two extra pages at most. A completed scan restarts from the recent window's tail,
/// eventually covering bursts larger than that window without an unbounded refresh.
pub(super) async fn items(
    client: &mut Client,
    id: &str,
    scan: &mut Scan,
    recent_tail: Option<String>,
) -> bool {
    scan.mode(Mode::Items);
    if recent_tail.as_ref().is_some_and(|c| c.len() > MAX_CURSOR) {
        return true;
    }
    if scan.cursor.is_none() {
        scan.cursor = recent_tail;
    }
    if scan.cursor.is_none() {
        return scan.limited;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
    for _ in 0..2 {
        if std::time::Instant::now() >= deadline {
            return true;
        }
        let Ok(page) = client
            .call(
                "thread/items/list",
                json!({"threadId":id,"limit":64,"sortDirection":"desc","cursor":scan.cursor}),
            )
            .await
        else {
            return true;
        };
        let Some(data) = page.get("data").and_then(Value::as_array) else {
            return true;
        };
        scan.items(data.iter().take(64).filter_map(|entry| entry.get("item")));
        if data.len() > 64 || !scan.advance(&page) {
            return true;
        }
        if scan.cursor.is_none() {
            break;
        }
    }
    scan.limited
}

/// Older servers expose full turns but no item API. Read one chronological turn per
/// page; process at most 256 items, continuing a long turn by its stable oldest offset.
pub(super) async fn turns(client: &mut Client, id: &str, scan: &mut Scan) -> bool {
    scan.mode(Mode::Turns);
    let Ok(page) = client.call("thread/turns/list", json!({"threadId":id,"limit":1,"sortDirection":"asc","itemsView":"full","cursor":scan.cursor})).await else { return true; };
    let Some(data) = page.get("data").and_then(Value::as_array) else {
        return true;
    };
    let Some(turn) = data.first() else {
        scan.cursor = None;
        scan.offset = 0;
        return scan.limited;
    };
    if turn
        .get("itemsView")
        .and_then(Value::as_str)
        .is_some_and(|v| v != "full")
    {
        return true;
    }
    let Some(items) = turn.get("items").and_then(Value::as_array) else {
        return true;
    };
    if scan.offset >= items.len() {
        scan.offset = 0;
    }
    scan.items(items.iter().skip(scan.offset).take(ITEMS));
    scan.offset = scan.offset.saturating_add(ITEMS).min(items.len());
    if scan.offset == items.len() && !scan.advance(&page) {
        return true;
    }
    data.len() > 1 || scan.limited
}
