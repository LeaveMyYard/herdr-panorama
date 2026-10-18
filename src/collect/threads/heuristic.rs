//! Threads for the top-level agents no stronger source claimed (Python
//! `threads.group_refs_and_turns`): by the PR or issue their descriptions name, else by the
//! operator turn that started them, each with its heuristic phases ([`super::phases`]).

use super::Draft;
use super::phases::heuristic_phases;
use super::roles::ref_key;
use super::spans::{Span, own_span};
use crate::collect::agents::Arena;
use indexmap::IndexMap;
use panorama_core::model::ThreadSource;
use std::collections::HashMap;

/// A heuristic thread of these agents.
fn heuristic_thread(
    id: String,
    name: String,
    source: ThreadSource,
    ids: Vec<String>,
    spans_of: &HashMap<String, Vec<Span>>,
    arena: &mut Arena,
) -> Draft {
    let mut spans = Vec::new();
    for id in &ids {
        let own = spans_of
            .get(id)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| {
                let a = &arena[id];
                vec![own_span(a, a.started)]
            });
        spans.extend(own.into_iter().map(|s| (id.clone(), s)));
    }
    let mut d = Draft::new(id, name, source);
    d.thread.phases = heuristic_phases(spans, arena);
    d.ids = ids;
    d
}

/// Threads for the unclaimed top-level agents: one per reference (a bare `#123` joins the one
/// `<repo>#123` thread when exactly one exists), then one per operator turn.
pub fn group_refs_and_turns(
    top: &[String],
    turn_of: &HashMap<String, i64>,
    spans_of: &HashMap<String, Vec<Span>>,
    arena: &mut Arena,
) -> Vec<Draft> {
    let mut keyed: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    let mut rest: Vec<String> = Vec::new();
    for id in top {
        match ref_key(&arena[id].description) {
            Some((k, name)) => {
                keyed.entry(k.clone()).or_default().push(id.clone());
                names.entry(k).or_insert(name);
            }
            None => rest.push(id.clone()),
        }
    }
    let bare: Vec<String> = keyed
        .keys()
        .filter(|k| k.starts_with('#'))
        .cloned()
        .collect();
    for key in bare {
        let full: Vec<String> = keyed
            .keys()
            .filter(|k| !k.starts_with('#') && k.ends_with(key.as_str()))
            .cloned()
            .collect();
        if let [one] = full.as_slice() {
            let moved = keyed.shift_remove(&key).unwrap_or_default();
            keyed[one].extend(moved);
        }
    }
    let mut out = Vec::new();
    for (key, ids) in keyed {
        let name = names[&key].clone();
        out.push(heuristic_thread(
            format!("ref:{key}"),
            name,
            ThreadSource::Ref,
            ids,
            spans_of,
            arena,
        ));
    }
    let mut turns: IndexMap<i64, Vec<String>> = IndexMap::new();
    for id in rest {
        turns
            .entry(turn_of.get(&id).copied().unwrap_or(-1))
            .or_default()
            .push(id);
    }
    for (idx, mut ids) in turns {
        ids.sort_by(|a, b| {
            arena[a]
                .started
                .unwrap_or(0.0)
                .total_cmp(&arena[b].started.unwrap_or(0.0))
        });
        let first = &arena[&ids[0]];
        let mut name = if first.description.is_empty() {
            first.id.clone()
        } else {
            first.description.clone()
        };
        if ids.len() > 1 {
            name += &format!(" +{}", ids.len() - 1);
        }
        out.push(heuristic_thread(
            format!("turn:{idx}"),
            name,
            ThreadSource::Turn,
            ids,
            spans_of,
            arena,
        ));
    }
    out
}

#[cfg(test)]
mod tests;
