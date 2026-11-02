//! Fixed Rust expectations for the authorized harness labels and Codex launcher support.
//! Python/design captures stay immutable. Exact original rows select explicit replacement
//! rows; all cells outside those reviewed changes are still compared normally.
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

fn normalized(row: &Value) -> Option<String> {
    let mut runs: Vec<Value> = vec![];
    for r in row.as_array()? {
        let r = r.as_array().filter(|r| r.len() == 4)?;
        let text = r[0].as_str()?;
        if !r[1].is_null() && !r[1].is_string() { return None; }
        if !r[2].is_null() && !r[2].is_string() { return None; }
        if !r[3].is_boolean() { return None; }
        for ch in text.chars() {
            let fg = if ch == ' ' {None} else {Some(r[1].as_str().unwrap_or("#c3c7cd").to_lowercase())};
            let bg = r[2].as_str().unwrap_or("#101215").to_lowercase();
            let bold = ch != ' ' && r[3].as_bool().unwrap();
            if let Some(last) = runs.last_mut() {
                if last[1] == json!(fg) && last[2] == json!(bg) && last[3] == json!(bold) {
                    last[0] = json!(last[0].as_str().unwrap().to_owned() + &ch.to_string());
                    continue;
                }
            }
            runs.push(json!([ch.to_string(), fg, bg, bold]));
        }
    }
    Some(serde_json::to_string(&runs).unwrap())
}

fn changes() -> &'static HashMap<String, (String, Value)> {
    static CHANGES: OnceLock<HashMap<String, (String, Value)>> = OnceLock::new();
    CHANGES.get_or_init(|| {
        let rows: Vec<Value> = serde_json::from_str(include_str!("rows.json")).unwrap();
        let mut changes = HashMap::new();
        for row in rows {
            let before = normalized(&row["before"]).unwrap();
            for path in row["fixtures"].as_array().unwrap() {
                assert!(changes.insert(path.as_str().unwrap().into(), (before.clone(), row["after"].clone())).is_none());
            }
        }
        changes
    })
}

pub fn adapt(rel: &str, mut fixture: Value) -> Value {
    fn walk(v: &mut Value, path: &str) {
        if let Some((before, replacement)) = changes().get(path) {
            assert_eq!(normalized(v).as_ref(), Some(before), "harness delta no longer matches {path}");
            *v = replacement.clone();
            return;
        }
        match v {
            Value::Array(xs) => { for (i,x) in xs.iter_mut().enumerate() { walk(x, &format!("{path}/{i}")); } },
            Value::Object(xs) => { for (k,x) in xs { walk(x, &format!("{path}/{k}")); } },
            _ => {}
        }
    }
    walk(&mut fixture, &format!("tests/fixtures/{rel}"));
    fixture
}
