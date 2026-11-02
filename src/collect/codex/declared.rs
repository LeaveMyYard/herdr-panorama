//! Exact-source report scopes. Root reports win; a child can claim only itself
//! and verified descendants. Source IDs namespace otherwise identical reports.

use super::channels::Reports;
use crate::collect::agents::Arena;
use crate::collect::threads::{Draft, declared::declared_thread, finish};
use panorama_core::model::ThreadStatus;
use serde_json::json;
use std::collections::{HashMap, HashSet};

pub(super) fn partition(
    reports: &[Reports],
    root: &str,
    arena: &mut Arena,
    now: f64,
) -> (Vec<Draft>, HashSet<String>) {
    let mut claimed = HashSet::new();
    let mut drafts = Vec::new();
    for report in reports {
        let mut scoped: Arena = arena
            .iter()
            .filter(|(id, _)| {
                !claimed.contains(*id)
                    && (report.source == root || within(id, &report.source, arena))
            })
            .map(|(id, agent)| (id.clone(), agent.clone()))
            .collect();
        for declaration in &report.declarations {
            let mut declaration = declaration.clone();
            if report.source != root && declaration.agents.is_empty() {
                declaration
                    .agents
                    .push(json!({"match": report.source}).as_object().unwrap().clone());
            }
            let mut draft = declared_thread(&declaration, &mut scoped, &mut claimed, now);
            draft.thread.id = format!("x:codex:{}:{}", report.source, declaration.thread);
            for id in &draft.ids {
                arena[id] = scoped[id].clone();
            }
            drafts.push(draft);
        }
        inherit(&mut drafts, arena, &mut claimed);
    }
    for draft in &mut drafts {
        if draft.ids.is_empty() && finish::unset(&draft.thread.status) {
            draft.thread.status = ThreadStatus::from_code("unknown");
        }
        finish::finish(draft, arena);
    }
    (drafts, claimed)
}

fn within(id: &str, source: &str, arena: &Arena) -> bool {
    let mut id = id;
    let mut visited = HashSet::new();
    while visited.insert(id) {
        if id == source {
            return true;
        }
        let Some(parent) = arena.get(id).and_then(|agent| agent.parent.as_deref()) else {
            break;
        };
        id = parent;
    }
    false
}

fn inherit(drafts: &mut [Draft], arena: &Arena, claimed: &mut HashSet<String>) {
    let mut owners: HashMap<String, usize> = drafts
        .iter()
        .enumerate()
        .flat_map(|(index, draft)| draft.ids.iter().map(move |id| (id.clone(), index)))
        .collect();
    for (id, agent) in arena {
        if owners.contains_key(id) {
            continue;
        }
        if let Some(index) = agent
            .parent
            .as_ref()
            .and_then(|parent| owners.get(parent))
            .copied()
        {
            drafts[index].ids.push(id.clone());
            owners.insert(id.clone(), index);
            claimed.insert(id.clone());
        }
    }
}
