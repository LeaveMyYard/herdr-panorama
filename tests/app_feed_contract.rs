//! Executable field tables and identity invariants for the app feed; no live IO.

use panorama_core::model::Swarm;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

const DOC: &str = include_str!("../docs/protocol/app-feed-v1.md");
const SWARM: &str = include_str!("fixtures/app-feed/swarm.json");
const DEGRADED: &str = include_str!("fixtures/app-feed/degraded.json");
type Schema<'a> = BTreeMap<&'a str, Vec<(&'a str, &'a str)>>;

fn schema() -> Schema<'static> {
    let mut schema = BTreeMap::new();
    let mut section = "";
    for line in DOC.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            section = name;
        }
        if line.starts_with("| ") {
            let parts: Vec<_> = line.split('|').map(str::trim).collect();
            if parts[1] != "Field" {
                schema
                    .entry(section)
                    .or_insert_with(Vec::new)
                    .push((parts[1], parts[2]));
            }
        }
    }
    assert_eq!(schema.len(), 7, "all documented objects must be checked");
    schema
}

fn typed(value: &Value, kind: &str, schema: &Schema<'_>) -> Result<(), String> {
    if let Some(inner) = kind.strip_prefix('?') {
        return if value.is_null() {
            Ok(())
        } else {
            typed(value, inner, schema)
        };
    }
    if let Some(inner) = kind.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        let arr = value.as_array().ok_or(format!("expected {kind}"))?;
        for item in arr {
            typed(item, inner, schema)?;
        }
        return Ok(());
    }
    let scalar = match kind {
        "string" => Some(value.is_string()),
        "integer" => Some(value.as_f64().is_some_and(|n| {
            n.is_finite() && n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0
        })),
        "number" => Some(value.as_f64().is_some_and(f64::is_finite)),
        "bool" => Some(value.is_boolean()),
        _ => None,
    };
    if let Some(valid) = scalar {
        return valid.then_some(()).ok_or(format!("expected {kind}"));
    }
    let fields = schema.get(kind).ok_or(format!("unknown type {kind}"))?;
    let obj = value.as_object().ok_or(format!("expected {kind} object"))?;
    for (field, ty) in fields {
        let value = obj.get(*field).ok_or(format!("missing {kind}.{field}"))?;
        typed(value, ty, schema).map_err(|e| format!("{kind}.{field}: {e}"))?;
    }
    Ok(())
}

fn normalize(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        Value::Object(fields) => {
            Value::Object(fields.into_iter().map(|(k, v)| (k, normalize(v))).collect())
        }
        Value::Number(ref n) if n.as_i64().is_none() => {
            let number = n.as_f64().unwrap();
            if number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0 {
                json!(number as i64)
            } else {
                value
            }
        }
        _ => value,
    }
}

fn unique(values: &[Value]) -> bool {
    values
        .iter()
        .map(Value::to_string)
        .collect::<HashSet<_>>()
        .len()
        == values.len()
}

fn validate(feed: &Value) -> Result<(), String> {
    typed(feed, "Feed", &schema())?;
    if feed["v"].as_f64() != Some(1.0) {
        return Err("unsupported version".into());
    }
    // serde model integers are lexical; the JS wire contract is numeric.
    let normalized = normalize(feed.clone());
    let feed = &normalized;
    let swarm: Swarm = serde_json::from_value(feed.clone()).map_err(|e| e.to_string())?;
    let counts = serde_json::to_value(swarm.counts()).unwrap();
    for (field, _) in &schema()["Counts"] {
        if counts[*field] != feed["counts"][*field] {
            return Err(format!("legacy count {field} differs"));
        }
    }
    let mut roots = HashSet::new();
    for root in feed["orchestrators"].as_array().unwrap() {
        if root["hidden_done"].as_i64().unwrap() < 0 {
            return Err("negative hidden_done".into());
        }
        let host = root["host"].as_str().unwrap();
        if !["herdr-core", "herdr-tui", "unknown"].contains(&host) {
            return Err("invalid host".into());
        }
        let terminal = root["terminal"].as_str();
        let pane = root["pane_id"].as_str().unwrap();
        if pane.is_empty() || terminal == Some("") || root["harness"] == "" {
            return Err("empty identity".into());
        }
        let address = if host == "herdr-core" {
            terminal.unwrap_or(pane)
        } else {
            pane
        };
        if !roots.insert((host, address)) {
            return Err("duplicate root".into());
        }
        if host == "herdr-core"
            && (root["tab_id"] != Value::Null
                || root["workspace_id"] != Value::Null
                || root["tab_no"] != Value::Null
                || root["focused"] != false)
        {
            return Err("core contains app membership".into());
        }
        let inbox = root["inbox"].as_array().unwrap();
        let texts = root["inbox_texts"].as_array().unwrap();
        if inbox.len() != texts.len() || !unique(inbox) {
            return Err("invalid root questions".into());
        }
        if feed["inbox_ok"] == false && !inbox.is_empty() {
            return Err("unavailable inbox has questions".into());
        }
        let mut threads = HashSet::new();
        let mut agents = HashSet::new();
        for thread in root["threads"].as_array().unwrap() {
            let id = thread["id"].as_str().unwrap();
            if id.is_empty() || !threads.insert(id) {
                return Err("invalid thread identity".into());
            }
            for agent in thread["agents"].as_array().unwrap() {
                for field in ["tool_calls", "resumes"] {
                    if agent[field].as_i64().unwrap() < 0 {
                        return Err(format!("negative {field}"));
                    }
                }
                let id = agent["id"].as_str().unwrap();
                if id.is_empty() || !agents.insert(id) || agent["harness"] == "" {
                    return Err("invalid agent identity".into());
                }
                let qids = agent["question_ids"].as_array().unwrap();
                let questions = agent["questions"].as_array().unwrap();
                if qids.len() != questions.len() || !unique(qids) {
                    return Err("invalid agent questions".into());
                }
                for (id, text) in qids.iter().zip(questions) {
                    let at = inbox.iter().position(|q| q == id);
                    if at.is_none_or(|at| texts[at] != *text) {
                        return Err("question attribution differs".into());
                    }
                }
                for phase in agent["phases"].as_array().unwrap() {
                    if phase
                        .as_u64()
                        .is_none_or(|i| i as usize >= thread["phases"].as_array().unwrap().len())
                    {
                        return Err("invalid phase index".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn fixture() -> Value {
    serde_json::from_str(SWARM).unwrap()
}

#[test]
fn fixtures_validate_against_the_documented_field_tables_and_model_counts() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-feed");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let feed: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            validate(&feed).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            names.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    names.sort();
    assert_eq!(names, ["degraded.json", "swarm.json"]);
    let degraded: Value = serde_json::from_str(DEGRADED).unwrap();
    assert_eq!(degraded["herdr_ok"], false);
    assert_eq!(degraded["inbox_ok"], false);
    assert!(!degraded["errors"].as_array().unwrap().is_empty());
}

#[test]
fn fixture_exercises_deep_mixed_harness_statuses_and_question_ownership() {
    let feed = fixture();
    let swarm: Swarm = serde_json::from_value(feed.clone()).unwrap();
    let root = &feed["orchestrators"][0];
    let agents = root["threads"][0]["agents"].as_array().unwrap();
    let statuses: HashSet<_> = agents
        .iter()
        .map(|a| a["status"].as_str().unwrap())
        .collect();
    for code in [
        "q",
        "run",
        "rev",
        "wait",
        "blk",
        "fail",
        "done",
        "cxl",
        "inf",
        "lost",
        "bg",
        "stale",
        "future-status",
    ] {
        assert!(statuses.contains(code), "missing status {code}");
    }
    assert!(
        agents
            .iter()
            .map(|a| &a["harness"])
            .collect::<HashSet<_>>()
            .len()
            >= 4
    );
    let thread = &swarm.orchestrators[0].threads[0];
    let tree = panorama_core::model::TreeKids::of(thread);
    let mut parent = None;
    for depth in 1..=4 {
        let child = tree.children(parent).first().unwrap();
        assert_eq!(child.depth, depth);
        parent = Some(child.id.as_str());
    }
    assert!(agents.iter().any(|a| a["status"] == "fail"
        && a["error"].is_string()
        && a["transcript"].is_string()
        && !a["last_message"].as_str().unwrap().is_empty()));
    let mentions: usize = agents
        .iter()
        .map(|a| a["question_ids"].as_array().unwrap().len())
        .sum();
    assert_eq!(mentions, 2);
    assert_eq!(root["inbox"], json!([14, 15])); // Q14 is attributed twice; Q15 is root-only.
    assert!(
        swarm
            .orchestrators
            .iter()
            .any(|o| o.transcript.is_none() && o.threads.is_empty())
    );
}

#[test]
fn only_the_agreed_additions_differ_from_existing_snapshot_json() {
    let mut feed = fixture();
    let swarm: Swarm = serde_json::from_value(feed.clone()).unwrap();
    feed.as_object_mut().unwrap().remove("v");
    for root in feed["orchestrators"].as_array_mut().unwrap() {
        root.as_object_mut().unwrap().remove("host");
        root.as_object_mut().unwrap().remove("terminal");
        for thread in root["threads"].as_array_mut().unwrap() {
            for agent in thread["agents"].as_array_mut().unwrap() {
                agent.as_object_mut().unwrap().remove("harness");
            }
        }
    }
    let rendered = panorama::snapshot::render(&swarm, panorama::snapshot::Format::Json, 0.0);
    let snapshot: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(feed, snapshot);
}

#[test]
fn invalid_versions_types_and_required_fields_are_rejected() {
    for version in [Value::Null, json!(true), json!(1.5), json!("1"), json!(2)] {
        let mut feed = fixture();
        feed["v"] = version;
        assert!(validate(&feed).is_err());
    }
    for pointer in [
        "/v",
        "/orchestrators/0/terminal",
        "/orchestrators/0/threads/0/agents/0/harness",
    ] {
        let mut feed = fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        feed.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(validate(&feed).is_err(), "{pointer}");
    }
    for pointer in [
        "/errors",
        "/counts/agents",
        "/orchestrators/0/threads/0/phases/0/start",
        "/orchestrators/0/threads/0/agents/0/parent",
    ] {
        let mut feed = fixture();
        *feed.pointer_mut(pointer).unwrap() = json!({});
        assert!(validate(&feed).is_err(), "{pointer}");
    }
}

#[test]
fn identity_membership_and_question_mutations_are_rejected() {
    for (pointer, value) in [
        ("/orchestrators/1/terminal", json!("t2")),
        ("/orchestrators/0/tab_id", json!("app-tab")),
        ("/orchestrators/0/threads/1/id", json!("thread-a")),
        ("/orchestrators/0/threads/0/agents/1/id", json!("agent-0")),
        (
            "/orchestrators/0/threads/0/agents/1/question_ids/0",
            json!(999),
        ),
        (
            "/orchestrators/0/threads/0/agents/1/questions/0",
            json!("wrong text"),
        ),
        ("/orchestrators/0/threads/0/agents/0/phases/0", json!(99)),
        ("/counts/questions", json!(4)),
    ] {
        let mut feed = fixture();
        *feed.pointer_mut(pointer).unwrap() = value;
        assert!(validate(&feed).is_err(), "{pointer}");
    }
}

#[test]
fn additive_fields_unknown_vocabulary_and_scoped_ids_remain_compatible() {
    let mut feed = fixture();
    feed["future_field"] = json!({"anything": true});
    feed["counts"]["future_count"] = json!(123);
    feed["orchestrators"][0]["future_root"] = json!(true);
    feed["orchestrators"][0]["threads"][0]["future_thread"] = json!(true);
    feed["orchestrators"][0]["threads"][0]["phases"][0]["future_phase"] = json!(true);
    feed["orchestrators"][0]["threads"][0]["agents"][0]["future_agent"] = json!(true);
    feed["orchestrators"][0]["threads"][0]["agents"][8]["bg"][0]["future_task"] = json!(true);
    feed["orchestrators"][0]["harness"] = json!("another-harness");
    feed["orchestrators"][0]["threads"][0]["agents"][0]["status"] = json!("new-status");
    feed["counts"]["running"] = json!(1);
    validate(&feed).unwrap();
    // The same agent/thread ids are safe in a different root. Question ids are scoped too.
    feed["orchestrators"][1]["threads"] = feed["orchestrators"][0]["threads"].clone();
    feed["orchestrators"][1]["inbox"] = feed["orchestrators"][0]["inbox"].clone();
    feed["orchestrators"][1]["inbox_texts"] = feed["orchestrators"][0]["inbox_texts"].clone();
    let swarm: Swarm = serde_json::from_value(feed.clone()).unwrap();
    feed["counts"] = serde_json::to_value(swarm.counts()).unwrap();
    validate(&feed).unwrap();
}

#[test]
fn integer_validation_matches_javascript_safe_numeric_semantics() {
    for source in ["1", "1.0", "1e0"] {
        let mut feed = fixture();
        feed["v"] = serde_json::from_str(source).unwrap();
        feed["counts"]["agents"] = json!(13.0);
        validate(&feed).unwrap();
    }
    for value in [
        json!(true),
        json!(0.5),
        json!(9_007_199_254_740_992_u64),
        json!(-9_007_199_254_740_992_i64),
    ] {
        let mut feed = fixture();
        feed["orchestrators"][0]["threads"][0]["agents"][0]["seq"] = value;
        assert!(validate(&feed).is_err());
    }
}

#[test]
fn fixture_pins_needs_items_separately_from_distinct_questions() {
    let feed = fixture();
    let items = |root: &Value| {
        let detailed = root["threads"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|t| t["agents"].as_array().unwrap())
            .filter(|a| matches!(a["status"].as_str(), Some("fail" | "blk" | "wait")))
            .count();
        detailed
            + usize::from(
                detailed == 0
                    && (root["herdr_status"] == "blocked"
                        || !root["inbox"].as_array().unwrap().is_empty()),
            )
    };
    let roots = feed["orchestrators"].as_array().unwrap();
    assert_eq!(
        roots.iter().map(items).collect::<Vec<_>>(),
        [3, 1, 0, 0, 0, 0]
    );
    // Two detailed mentions of Q14 plus root-only Q15 still mean two questions.
    assert_eq!(feed["counts"]["questions"], 2);
    let mut root = roots[0].clone();
    root["herdr_status"] = json!("blocked");
    assert_eq!(items(&root), 3);
    root["threads"] = json!([]);
    assert_eq!(items(&root), 1);
    root["herdr_status"] = json!("idle");
    assert_eq!(items(&root), 1);
}

#[test]
fn documented_nonnegative_counts_reject_negative_values_and_allow_zero() {
    for pointer in [
        "/orchestrators/0/hidden_done",
        "/orchestrators/0/threads/0/agents/0/tool_calls",
        "/orchestrators/0/threads/0/agents/0/resumes",
    ] {
        let mut feed = fixture();
        *feed.pointer_mut(pointer).unwrap() = json!(-1);
        assert!(validate(&feed).is_err(), "{pointer}");
        *feed.pointer_mut(pointer).unwrap() = json!(0);
        validate(&feed).unwrap();
    }
}
