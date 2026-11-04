//! Python-captured swarms (`tests/fixtures/inputs/swarm-*.json`, `graph_rules.json`: panel
//! protocol 1.x, `orchestrators` → `threads` → `agents`) in this model's 2.x wire form
//! (`docs/protocol/panel-v2.md`): `roots` with one flat `agents` list each.
//!
//! The captures stay Python's own record (plan `uniform-agent-tree.md` R3, D7); the Rust tests
//! read them through this, the way a uniform-tree collector builds a root (U3):
//! - every thread's agents, in order, become the root's `agents`, without `phases`;
//! - the session card's `main:` agent goes, its state is the root's own;
//! - `threads`, `flavour` and `hidden_done` go.
//!
//! A 2.x swarm (already `roots`) comes back as it is.
#![allow(dead_code)] // each test binary uses its own part

use serde_json::Value;

/// `swarm` (a `state.swarm` or a `snapshot --json`) in the 2.x form.
pub fn swarm(mut swarm: Value) -> Value {
    if let Some(obj) = swarm.as_object_mut()
        && let Some(orchs) = obj.remove("orchestrators")
    {
        let roots = orchs.as_array().into_iter().flatten().cloned().map(root);
        obj.insert("roots".into(), Value::Array(roots.collect()));
    }
    swarm
}

/// A captured fixture whose `swarm` (if any) is read in the 2.x form.
pub fn fixture(mut fixture: Value) -> Value {
    if let Some(sw) = fixture.get_mut("swarm") {
        *sw = swarm(sw.take());
    }
    fixture
}

fn root(mut o: Value) -> Value {
    let Some(obj) = o.as_object_mut() else {
        return o;
    };
    let threads = obj.remove("threads");
    obj.remove("flavour");
    obj.remove("hidden_done");
    let agents = threads
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|t| t["agents"].as_array().into_iter().flatten())
        .filter(|a| !a["id"].as_str().is_some_and(|id| id.starts_with("main:")))
        .cloned()
        .map(|mut a| {
            if let Some(a) = a.as_object_mut() {
                a.remove("phases");
            }
            a
        });
    obj.insert("agents".into(), Value::Array(agents.collect()));
    o
}
