//! The panel's width in the split it shares with its agent (Python `cockpit.fit`): its
//! drawable width is `cockpit_width` columns, its rect that plus herdr's pane frame.

use serde_json::Value;

/// `cockpit_width` when unset.
pub const DEFAULT_WIDTH: i64 = 46;
/// herdr's pane frame: the columns a pane's border takes.
pub const FRAME: i64 = 2;

/// `cockpit_width` as set, clamped (36: the header and footer fit whole; under 120: the
/// narrow layout).
pub fn width(setting: Option<&Value>) -> i64 {
    let w = setting
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_f64().map(|f| f as i64))
                .or_else(|| v.as_str()?.trim().parse().ok())
        })
        .unwrap_or(DEFAULT_WIDTH);
    w.clamp(36, 119)
}

/// The split that holds `pane` as a direct child: (its path, the split, the pane is first).
pub fn split_of<'a>(
    node: &'a Value,
    pane: &str,
    path: &mut Vec<bool>,
) -> Option<(Vec<bool>, &'a Value, bool)> {
    if node.get("type")?.as_str()? != "split" {
        return None;
    }
    for (second, key) in [(false, "first"), (true, "second")] {
        let child = &node[key];
        if child["type"] == "pane" && child["pane_id"] == pane {
            return Some((path.clone(), node, !second));
        }
    }
    for (second, key) in [(false, "first"), (true, "second")] {
        path.push(second);
        let found = split_of(&node[key], pane, path);
        path.pop();
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Every pane id under `node`.
pub fn pane_ids(node: &Value) -> Vec<String> {
    match node.get("type").and_then(Value::as_str) {
        Some("pane") => node["pane_id"]
            .as_str()
            .map(str::to_owned)
            .into_iter()
            .collect(),
        Some(_) => [&node["first"], &node["second"]]
            .into_iter()
            .flat_map(pane_ids)
            .collect(),
        None => vec![],
    }
}

/// The ratio to set for `pane` given the tab's layout and the panes' rects, as `(path,
/// ratio)`; `None` when there is no right split to size (alone in its tab, a down split, no
/// rects).
pub fn ratio(root: &Value, rects: &Value, pane: &str, width: i64) -> Option<(Vec<bool>, f64)> {
    let (path, node, first) = split_of(root, pane, &mut vec![])?;
    if node["direction"] != "right" {
        return None;
    }
    let ids = pane_ids(node);
    let mine: Vec<(i64, i64)> = rects
        .as_array()?
        .iter()
        .filter(|p| {
            p["pane_id"]
                .as_str()
                .is_some_and(|i| ids.iter().any(|x| x == i))
        })
        .map(|p| {
            (
                p["rect"]["x"].as_i64().unwrap_or(0),
                p["rect"]["width"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    let total = mine.iter().map(|(x, w)| x + w).max()? - mine.iter().map(|(x, _)| *x).min()?;
    if total <= 0 {
        return None;
    }
    let share = ((width + FRAME) as f64 / total as f64).clamp(0.05, 0.95);
    Some((path, if first { share } else { 1.0 - share }))
}
