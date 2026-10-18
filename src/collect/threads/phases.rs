//! A heuristic thread's phases (Python `threads.heuristic_phases`): one phase per run of
//! consecutive same-role spans by start; reviews and fixes are numbered by round.

use super::roles::phase_word;
use super::spans::Span;
use crate::collect::agents::Arena;
use crate::collect::py::truthy_time;
use panorama_core::model::{Phase, PhaseState, Status};
use std::collections::HashMap;

/// One phase per run of consecutive same-role spans (by start); each agent gets the indexes
/// of its phases.
pub fn heuristic_phases(mut spans: Vec<(String, Span)>, arena: &mut Arena) -> Vec<Phase> {
    spans.sort_by(|a, b| {
        a.1.start
            .unwrap_or(0.0)
            .total_cmp(&b.1.start.unwrap_or(0.0))
    });
    // (the role word, its spans) per phase, and the phase names
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (si, (id, _)) in spans.iter().enumerate() {
        let word = phase_word(&arena[id].role).to_owned();
        if groups.last().is_none_or(|(w, _)| *w != word) {
            let n = *counts
                .entry(word.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            let name = match (word.as_str(), n) {
                ("review" | "fix", n) => format!("{word} r{n}"),
                (_, 1) => word.clone(),
                (_, n) => format!("{word} {n}"),
            };
            groups.push((word.clone(), vec![si]));
            names.push(name);
        } else if let Some(g) = groups.last_mut() {
            g.1.push(si);
        }
        let idx = groups.len() as i64 - 1;
        let a = &mut arena[id];
        if !a.phases.contains(&idx) {
            a.phases.push(idx);
        }
    }
    let total = groups.len();
    groups
        .into_iter()
        .zip(names)
        .enumerate()
        .map(|(i, ((_, members), name))| phase(&name, &members, &spans, i + 1 == total))
        .collect()
}

fn phase(name: &str, members: &[usize], spans: &[(String, Span)], last_phase: bool) -> Phase {
    let sps: Vec<&Span> = members.iter().map(|k| &spans[*k].1).collect();
    let running = sps.iter().any(|s| s.end.is_none());
    let start = sps
        .iter()
        .filter_map(|s| truthy_time(s.start))
        .reduce(f64::min);
    let end = if running {
        None
    } else {
        sps.iter()
            .filter_map(|s| truthy_time(s.end))
            .reduce(f64::max)
    };
    let last = sps
        .iter()
        .fold(None::<&&Span>, |best, s| match best {
            Some(b) if b.end.unwrap_or(0.0) >= s.end.unwrap_or(0.0) => Some(b),
            _ => Some(s),
        })
        .expect("a phase has a span");
    let state = if running {
        PhaseState::Now
    } else if last.outcome == Status::Failed && last_phase {
        PhaseState::Failed
    } else if sps.iter().all(|s| s.outcome == Status::Cancelled) {
        PhaseState::Skipped
    } else {
        PhaseState::Done
    };
    Phase {
        name: name.to_owned(),
        state,
        start,
        end,
        note: None,
    }
}

#[cfg(test)]
mod tests;
