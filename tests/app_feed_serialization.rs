use panorama::{
    app_feed::{Root, World, serialize},
    host::{Host, collect::Collected},
};
use panorama_core::model::Swarm;
use serde_json::{Value, json};
use std::collections::HashMap;

/// The world a v2 feed was written from: its roots and their agent trees, as the model reads
/// them, with each root's host, terminal and agent harnesses as provenance.
fn world(feed: &Value) -> World {
    let swarm: Swarm = serde_json::from_value(feed.clone()).unwrap();
    let roots = feed["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| Root {
            host: match o["host"].as_str().unwrap() {
                "herdr-core" => Host::Core,
                "herdr-tui" => Host::Tui,
                _ => Host::Unknown,
            },
            terminal: o["terminal"].as_str().map(str::to_owned),
            agent_harnesses: o["agents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    (
                        a["id"].as_str().unwrap().to_owned(),
                        a["harness"].as_str().unwrap().to_owned(),
                    )
                })
                .collect(),
        })
        .collect();
    World { swarm, roots }
}

#[test]
fn fixture_worlds_serialize_to_the_complete_normative_feed() {
    // every fixture, whole: the writer adds and drops nothing (plan `uniform-agent-tree.md` U7)
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-feed");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "json") {
            let expected: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let bytes = serialize::bytes(&world(&expected)).unwrap();
            assert!(bytes.ends_with(b"\n"));
            let got: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(got, expected, "{}", path.display());
            seen += 1;
        }
    }
    assert_eq!(seen, 3);
}

#[test]
fn harness_provenance_is_scoped_and_unknown_never_defaults_to_claude() {
    let feed: Value = serde_json::from_str(include_str!("fixtures/app-feed/swarm.json")).unwrap();
    let mut world = world(&feed);
    world.roots[0].agent_harnesses.clear();
    let model = serialize::model(&world).unwrap();
    assert_eq!(model["roots"][0]["agents"][0]["harness"], "unknown");
    world.roots[0]
        .agent_harnesses
        .insert("agent-0".into(), "future-harness".into());
    assert_eq!(
        serialize::model(&world).unwrap()["roots"][0]["agents"][0]["harness"],
        "future-harness"
    );
    // Reader-owned agents use their known format, without changing legacy model serialization.
    let collected = Collected {
        pending: false,
        swarm: world.swarm,
        host: Host::Core,
        terminals: HashMap::from([("core:t2".into(), "t2".into())]),
    };
    let known = World::from(collected);
    assert_eq!(known.roots[0].agent_harnesses["agent-0"], "codex");
    assert_eq!(known.roots[0].terminal.as_deref(), Some("t2"));
}

#[test]
fn core_membership_is_neutral_and_invalid_provenance_never_serializes() {
    let feed: Value = serde_json::from_str(include_str!("fixtures/app-feed/swarm.json")).unwrap();
    let mut world = world(&feed);
    world.swarm.roots[0].tab_id = Some("legacy-tab".into());
    world.swarm.roots[0].workspace_id = Some("legacy-space".into());
    world.swarm.roots[0].focused = true;
    let model = serialize::model(&world).unwrap();
    assert!(model["roots"][0]["tab_id"].is_null());
    assert!(model["roots"][0]["workspace_id"].is_null());
    assert_eq!(model["roots"][0]["focused"], false);
    world.swarm.generated_at = f64::NAN;
    assert!(serialize::bytes(&world).is_err());
    world.swarm.generated_at = 1.0;
    world.roots.pop();
    assert!(serialize::bytes(&world).is_err());
}

#[test]
fn unsafe_counts_and_duplicate_roots_are_rejected() {
    let feed: Value = serde_json::from_str(include_str!("fixtures/app-feed/swarm.json")).unwrap();
    let mut bad = feed.clone();
    bad["roots"][0]["agents"][0]["tool_calls"] = json!(9_007_199_254_740_992_i64);
    assert!(serialize::bytes(&world(&bad)).is_err());
    let mut bad = world(&feed);
    bad.roots[1].terminal = Some("t2".into());
    assert!(serialize::bytes(&bad).is_err());
}
